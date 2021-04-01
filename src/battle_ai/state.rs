use std::cmp::{max, min, Ordering};

use rand::prelude::StdRng;
use rand::Rng;

use crate::{choose_weighted_index, FieldPosition, MajorStatusAilment, Terrain, Weather, Counter};
use crate::battle_ai::{game_theory, pokemon};
use crate::battle_ai::game_theory::{Matrix, ZeroSumNashEq};
use crate::battle_ai::move_effects::Action;
use crate::battle_ai::pokemon::{Pokemon, TeamBuild};
use crate::move_::{Move, MoveCategory};

/// How many turns ahead the agents compute
pub const AI_LEVEL: u8 = 3;

/// Maximum number of times each agent is allowed to switch out Pokemon before it must choose a move
/// (does not count switching one in to replace a fainted team member)
const CONSECUTIVE_SWITCH_CAP: u16 = 2;

pub static mut NUM_STATE_COPIES: u64 = 0;

/// Represents the entire game state of a battle.
#[derive(Clone, Debug)]
pub struct State {
    /// ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team.
    pokemon: [Pokemon; 12],
    pub max: Agent,
    pub min: Agent,
    pub weather: Weather,
    pub weather_counter: Counter<u16>,
    pub terrain: Terrain,
    turn_number: u16,
    /// Battle print-out that is shown when this state is entered; useful for sanity checks.
    display_text: Vec<String>,
    children: Vec<Option<Box<State>>>,
}

impl State {
    fn new(pokemon: [Pokemon; 12], weather: Weather, terrain: Terrain) -> State {
        State {
            pokemon,
            max: Agent {
                on_field: None,
                actions: vec![
                    Action::Switch {
                        user_id: None,
                        switching_in_id: 6,
                        target_position: FieldPosition::Max
                    }
                ],
                action_order: vec![0],
                consecutive_switches: 0
            },
            min: Agent {
                on_field: None,
                actions: vec![
                    Action::Switch {
                        user_id: None,
                        switching_in_id: 0,
                        target_position: FieldPosition::Min
                    }
                ],
                action_order: vec![0],
                consecutive_switches: 0
            },
            weather,
            weather_counter: Counter::new(None),
            terrain,
            turn_number: 0,
            display_text: Vec::new(),
            children: vec![None; 1]
        }
    }

    pub const fn pokemon_by_id(&self, pokemon_id: u8) -> &Pokemon {
        &self.pokemon[pokemon_id as usize]
    }

    pub fn pokemon_by_id_mut(&mut self, pokemon_id: u8) -> &mut Pokemon {
        &mut self.pokemon[pokemon_id as usize]
    }

    pub fn add_display_text(&mut self, text: String) {
        self.display_text.push(text);
    }

    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    /// Copies only the game state into a new State instance; doesn't copy the child matrix or display text.
    fn copy_game_state(&self) -> State {
        unsafe { NUM_STATE_COPIES += 1; }

        State {
            pokemon: self.pokemon.clone(),
            max: Agent {
                on_field: self.max.on_field,
                actions: Vec::new(),
                action_order: Vec::new(),
                consecutive_switches: self.max.consecutive_switches
            },
            min: Agent {
                on_field: self.min.on_field,
                actions: Vec::new(),
                action_order: Vec::new(),
                consecutive_switches: self.min.consecutive_switches
            },
            weather: self.weather,
            weather_counter: self.weather_counter.clone(),
            terrain: self.terrain,
            turn_number: self.turn_number,
            display_text: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn has_battle_ended(&self) -> bool {
        self.pokemon[0..6].iter().all(|pokemon| pokemon.current_hp() == 0) || self.pokemon[6..12].iter().all(|pokemon| pokemon.current_hp() == 0)
    }

    /// Gets the specified child or generates it using this state's actions if it does not exist.
    ///
    /// Accesses children through the action orderings, giving the appearance that the child
    /// matrix is sorted by whichever actions are expected to produce the best outcome.
    fn get_or_gen_child(&mut self, i: usize, j: usize, rng: &mut StdRng) -> &mut State {
        let max_action_index = self.max.action_order[i];
        let min_action_index = self.min.action_order[j];
        let child_index = max_action_index * self.min.actions.len() + min_action_index;

        if self.children[child_index].is_none() {
            let mut child = self.copy_game_state();
            let max_action = &self.max.actions[max_action_index];
            let min_action = &self.min.actions[min_action_index];
            child.max.consecutive_switches = match max_action {
                Action::Switch { .. } => child.max.consecutive_switches + 1,
                _ => 0
            };
            child.min.consecutive_switches = match min_action {
                Action::Switch { .. } => child.min.consecutive_switches + 1,
                _ => 0
            };
            play_out_turn(&mut child, vec![max_action, min_action], rng);
            generate_actions(&mut child, rng);
            self.children[child_index] = Some(Box::new(child));
        }

        self.children[child_index].as_mut().unwrap()
    }

    /// Removes the specified child from this state.
    ///
    /// Accesses children through the action orderings, giving the appearance that the child
    /// matrix is sorted by whichever actions are expected to produce the best outcome.
    fn remove_child(&mut self, i: usize, j: usize, rng: &mut StdRng) -> Box<State> {
        self.get_or_gen_child(i, j, rng);

        let max_action_index = self.max.action_order[i];
        let min_action_index = self.min.action_order[j];
        let child_index = max_action_index * self.min.actions.len() + min_action_index;
        self.children.remove(child_index).unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct Agent {
    /// Pokemon owned by this agent that is on the field
    pub on_field: Option<u8>,
    actions: Vec<Action>,
    action_order: Vec<usize>,
    consecutive_switches: u16
}

/// Run a battle from an initial state; the maximizer and minimizer use game theory to choose their actions. All
/// state-space branching due to chance events during the course of each turn has been removed to reduce
/// computational complexity. Instead, one potential outcome is simply chosen at random (weighted appropriately),
/// so the agents behave as if they know the outcome ahead of time. Over many trials, the heuristic value should
/// average out to what one would obtain from a full state-space/probability tree search, but expect high variance
/// between individual trials. Returns a heuristic value between -1.0 and 1.0 signifying how well the maximizer did;
/// 0.0 would be a tie. The minimizer's value is its negation.
pub fn run_battle(minimizer: &TeamBuild, maximizer: &TeamBuild, rng: &mut StdRng) -> f64 {
    let mut state = Box::new(
        State::new({
                       let mut min_team = minimizer.members.iter();
                       let mut max_team = maximizer.members.iter();
                       [
                           Pokemon::from(min_team.next().unwrap()),
                           Pokemon::from(min_team.next().unwrap()),
                           Pokemon::from(min_team.next().unwrap()),
                           Pokemon::from(min_team.next().unwrap()),
                           Pokemon::from(min_team.next().unwrap()),
                           Pokemon::from(min_team.next().unwrap()),
                           Pokemon::from(max_team.next().unwrap()),
                           Pokemon::from(max_team.next().unwrap()),
                           Pokemon::from(max_team.next().unwrap()),
                           Pokemon::from(max_team.next().unwrap()),
                           Pokemon::from(max_team.next().unwrap()),
                           Pokemon::from(max_team.next().unwrap())
                       ]
                   }, Weather::default(), Terrain::default()));

    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE BEGIN >>>>");
        state.print_display_text();
    }

    let mut nash_eq = smab_search(&mut state, -1.0, 1.0, AI_LEVEL, rng);

    while !state.max.actions.is_empty() && !state.min.actions.is_empty() {
        let maximizer_choice = choose_weighted_index(&nash_eq.max_player_strategy, rng);
        let minimizer_choice = choose_weighted_index(&nash_eq.min_player_strategy, rng);

        let child = state.remove_child(maximizer_choice, minimizer_choice, rng);
        state = child;
        if cfg!(feature = "print-battle") { state.print_display_text(); }
        nash_eq = smab_search(&mut state, -1.0, 1.0, AI_LEVEL, rng);
    }

    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE END >>>>");
    }

    nash_eq.expected_payoff
}

/// Simultaneous move alpha-beta search, implemented as a simplification of
/// [Alpha-Beta Pruning for Games with Simultaneous Moves](docs/Alpha-Beta_Pruning_for_Games_with_Simultaneous_Moves.pdf).
fn smab_search(state: &mut State, mut alpha: f64, mut beta: f64, recursions: u8, rng: &mut StdRng) -> ZeroSumNashEq {
    let m = state.max.actions.len();
    let n = state.min.actions.len();

    // If depth limit reached or either agent has no actions, stop search.
    if recursions < 1 || m == 0 || n == 0 {
        return ZeroSumNashEq {
            max_player_strategy: vec![1.0 / m as f64; m],
            min_player_strategy: vec![1.0 / n as f64; n],
            expected_payoff: (state.pokemon[6..12].iter().map(|pokemon| pokemon.current_hp() as f64 / pokemon.max_hp() as f64).sum::<f64>()
                - state.pokemon[0..6].iter().map(|pokemon| pokemon.current_hp() as f64 / pokemon.max_hp() as f64).sum::<f64>()) / 6.0,
        };
    }

    let mut payoff_matrix = Matrix::of(0.0, m, n);
    let mut row_domination = vec![false; m];
    let mut col_domination = vec![false; n];
    let mut row_mins = vec![1.0; m];
    let mut col_maxes = vec![-1.0; n];

    let mut explore_child = |i: usize, j: usize| {
        if !row_domination[i] && !col_domination[j] {
            let child_value = smab_search(state.get_or_gen_child(i, j, rng), alpha, beta, recursions - 1, rng).expected_payoff;
            if child_value <= alpha {
                row_domination[i] = true;
            } else if child_value >= beta {
                col_domination[j] = true;
            } else {
                *payoff_matrix.get_mut(i, j) = child_value;
                if child_value < row_mins[i] { row_mins[i] = child_value; }
                if child_value > col_maxes[j] { col_maxes[j] = child_value; }
                if j == n - 1 && row_mins[i] > alpha { alpha = row_mins[i]; }
                if i == m - 1 && col_maxes[j] < beta { beta = col_maxes[j]; }
            }
        }
    };

    // Explore child matrix in an L-shape
    for d in 0..min(n, m) {
        for j in d..n {
            explore_child(d, j);
        }
        for i in (d + 1)..m {
            explore_child(i, d);
        }
    }

    if row_domination.iter().all(|b| *b) {
        return ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: alpha,
        };
    }

    if col_domination.iter().all(|b| *b) {
        return ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: beta,
        };
    }

    let nash_eq = game_theory::calc_nash_eq(&payoff_matrix, &row_domination, &col_domination, 2.0);

    // Exploiting iterative deepening: sort actions from highest probability of being played to
    // lowest probability so that alpha-beta is more likely to prune children on the next pass
    // through this state.
    let action_order_cmp = |strategy: &[f64], act_index1: &usize, act_index2: &usize| {
        let diff = strategy[*act_index1] - strategy[*act_index2];
        if almost::zero(diff) {
            Ordering::Equal
        } else if diff < 0.0 {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    };

    state.max.action_order.sort_unstable_by(|act_index1, act_index2| action_order_cmp(&nash_eq.max_player_strategy, act_index1, act_index2));
    state.min.action_order.sort_unstable_by(|act_index1, act_index2| action_order_cmp(&nash_eq.min_player_strategy, act_index1, act_index2));

    nash_eq
}

fn generate_actions(state: &mut State, rng: &mut StdRng) {
    match state.min.on_field.zip(state.max.on_field) {
        None => agents_choose_pokemon_to_send_out(state),
        Some((max_pokemon_id, min_pokemon_id)) => { // Agents must choose actions for each Pokemon
            let max_actions = gen_actions_for_user(state, rng, max_pokemon_id);
            let min_actions = gen_actions_for_user(state, rng, min_pokemon_id);
            state.max.actions = max_actions;
            state.min.actions = min_actions;

            // I don't know why reversing the comparator makes it run several times faster, but it
            // does, so I did.
            state.max.actions.sort_unstable_by(|act1, act2| action_cmp(act1, act2).reverse());
            state.min.actions.sort_unstable_by(|act1, act2| action_cmp(act1, act2).reverse());
        }
    }

    state.max.action_order = (0..state.max.actions.len()).collect();
    state.min.action_order = (0..state.min.actions.len()).collect();
    state.children = vec![None; state.max.actions.len() * state.min.actions.len()];
}

fn agents_choose_pokemon_to_send_out(state: &mut State) {
    state.max.actions = match state.max.on_field {
        None => (6..12)
            .filter(|id| state.pokemon_by_id(*id).current_hp() > 0)
            .map(|id| Action::Switch {
                user_id: None,
                switching_in_id: id,
                target_position: FieldPosition::Max,
            }).collect(),
        Some(_) => vec![Action::Nop]
    };

    state.min.actions = match state.min.on_field {
        None => (0..6)
            .filter(|id| state.pokemon_by_id(*id).current_hp() > 0)
            .map(|id| Action::Switch {
                user_id: None,
                switching_in_id: id,
                target_position: FieldPosition::Min,
            }).collect(),
        Some(_) => vec![Action::Nop]
    };
}

fn gen_actions_for_user(state: &mut State, rng: &mut StdRng, user_id: u8) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::with_capacity(9);

    // TODO: Is this actually what should happen?
    if let Some(next_move_action) = state.pokemon_by_id(user_id).next_move_action.clone() {
        if next_move_action.can_be_performed(state, rng) {
            actions.push(next_move_action);
            state.pokemon_by_id_mut(user_id).next_move_action = None;
            return actions;
        } else {
            state.pokemon_by_id_mut(user_id).next_move_action = None;
        }
    }

    let user = state.pokemon_by_id(user_id);
    for move_index in 0..user.known_moves().len() {
        if user.can_choose_move(move_index) {
            let move_ = user.known_move(move_index).move_();
            actions.push(Action::Move {
                user_id,
                move_,
                move_index: Some(move_index as u8),
                target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                    .filter(|field_pos| Move::targeting(move_).can_hit(user.field_position().unwrap(), *field_pos)).collect(),
            });
        }
    }

    // TODO: Can Struggle be used if switch actions are available?
    if actions.is_empty() {
        let struggle = Move::id_by_name("Struggle").unwrap();
        actions.push(Action::Move {
            user_id,
            move_: struggle,
            move_index: None,
            target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                .filter(|field_pos| Move::targeting(struggle).can_hit(user.field_position().unwrap(), *field_pos)).collect(),
        });
    }

    if (user_id < 6 && state.min.consecutive_switches < CONSECUTIVE_SWITCH_CAP) || (user_id >= 6 && state.max.consecutive_switches < CONSECUTIVE_SWITCH_CAP) {
        for team_member_id in if user_id < 6 { 0..6 } else { 6..12 } {
            let team_member = state.pokemon_by_id(team_member_id);
            if team_member.current_hp() > 0 && team_member.field_position() == None && team_member.known_moves().iter().map(|known_move| known_move.pp).sum::<u8>() > 0 {
                actions.push(Action::Switch {
                    user_id: Some(user_id),
                    switching_in_id: team_member_id as u8,
                    target_position: state.pokemon_by_id(user_id).field_position().unwrap(),
                });
            }
        }
    }

    actions
}

// TODO: Make better; order actions so that pruning is most likely to occur.
fn action_cmp(act1: &Action, act2: &Action) -> Ordering {
    match act1 {
        Action::Nop => Ordering::Greater,
        Action::Switch { .. } => {
            match act2 {
                Action::Nop => Ordering::Less,
                Action::Switch { .. } => Ordering::Equal,
                Action::Move { .. } => Ordering::Greater
            }
        }
        Action::Move { user_id: _, move_: act1_move, move_index: _, target_positions: _ } => {
            match act2 {
                Action::Move { user_id: _, move_: act2_move, move_index: _, target_positions: _ } => {
                    match Move::category(*act1_move) {
                        MoveCategory::Status => {
                            match Move::category(*act2_move) {
                                MoveCategory::Status => Ordering::Equal,
                                _ => Ordering::Greater
                            }
                        }
                        _ => {
                            match Move::category(*act2_move) {
                                MoveCategory::Status => Ordering::Less,
                                _ => Ordering::Equal
                            }
                        }
                    }
                }
                _ => Ordering::Less
            }
        }
    }
}

fn play_out_turn(state: &mut State, mut action_queue: Vec<&Action>, rng: &mut StdRng) {
    // Only advance turn counter if all agents are actually doing something
    if !action_queue.iter().any(|act| matches!(act, Action::Nop)) {
        if cfg!(feature = "print-battle") {
            let turn_number = state.turn_number;
            state.add_display_text(format!("---- Turn {} ----", turn_number));
        }

        for id in 0..12 {
            pokemon::increment_msa_counter(state, id);
        }

        state.turn_number += 1;
        if state.weather_counter.inc() {
            state.add_display_text(String::from(state.weather.display_text_on_disappearance()));
            state.weather = Weather::None;
        }
    }

    action_queue.sort_unstable_by(|act1, act2| Action::action_queue_ordering(state, rng, act1, act2));

    while !action_queue.is_empty() {
        let action = action_queue.remove(0);
        if action.can_be_performed(state, rng) && action.perform(state, &action_queue, rng) {
            return;
        }
    }

    // End of turn effects (order is randomized to avoid bias)
    let pokemon_on_field = if rng.gen_bool(0.5) {
        vec![state.min.on_field, state.max.on_field]
    } else {
        vec![state.max.on_field, state.min.on_field]
    };

    for on_field in pokemon_on_field {
        if let Some(on_field) = on_field {
            match state.pokemon[on_field as usize].major_status_ailment() {
                MajorStatusAilment::Poisoned => {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{} takes damage from poison!", state.pokemon[on_field as usize]);
                        state.add_display_text(display_text);
                    }
                    if pokemon::apply_damage(state, on_field, max(state.pokemon[on_field as usize].max_hp() / 8, 1) as i16) {
                        return;
                    }
                }
                MajorStatusAilment::BadlyPoisoned => {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{} takes damage from poison!", state.pokemon[on_field as usize]);
                        state.add_display_text(display_text);
                    }
                    let amount = {
                        let pokemon = state.pokemon_by_id(on_field);
                        ((pokemon.msa_counter.value + 1) * max(pokemon.max_hp() / 16, 1)) as i16
                    };
                    if pokemon::apply_damage(state, on_field, amount) {
                        return;
                    }
                }
                _ => {}
            }

            if let Some(seeder_pos) = state.pokemon[on_field as usize].seeded_by {
                let seeder_id = match seeder_pos {
                    FieldPosition::Min => state.min.on_field,
                    FieldPosition::Max => state.max.on_field
                };
                if let Some(seeder_id) = seeder_id {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{}'s seed drains energy from {}!", state.pokemon[seeder_id as usize], state.pokemon[on_field as usize]);
                        state.add_display_text(display_text);
                    }
                    let transferred_hp = max(state.pokemon[on_field as usize].max_hp() / 8, 1) as i16;
                    if pokemon::apply_damage(state, on_field, transferred_hp) || pokemon::apply_damage(state, seeder_id, -transferred_hp) {
                        return;
                    }
                }
            }
        }
    }
}
