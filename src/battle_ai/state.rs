use rand::prelude::StdRng;
use rand::Rng;
use std::cmp::{max, min, Ordering};
use crate::battle_ai::{game_theory, pokemon};
use crate::{FieldPosition, Weather, Terrain, choose_weighted_index, MajorStatusAilment};
use crate::battle_ai::move_effects::Action;
use crate::move_::{Move, MoveCategory};
use crate::battle_ai::pokemon::{Pokemon, TeamBuild};
use crate::battle_ai::game_theory::{ZeroSumNashEq, Matrix, calc_nash_eq};
use std::borrow::Borrow;

const AI_LEVEL: u8 = 3;

pub static mut NUM_STATE_COPIES: usize = 0;

/// Represents the entire game state of a battle.
#[derive(Clone, Debug)]
pub struct State {
    /// ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team.
    pokemon: [Pokemon; 12],
    /// Pokemon of the minimizing team that is on the field.
    pub min_pokemon_id: Option<u8>,
    /// Pokemon of the maximizing team that is on the field.
    pub max_pokemon_id: Option<u8>,
    pub weather: Weather,
    pub terrain: Terrain,
    turn_number: u16,
    /// Battle print-out that is shown when this state is entered; useful for sanity checks.
    display_text: Vec<String>,
    children: Vec<Option<State>>,
    max_actions: Vec<Box<Action>>,
    min_actions: Vec<Box<Action>>,
    max_action_order: Vec<usize>,
    min_action_order: Vec<usize>,
    max_consecutive_switches: u16,
    min_consecutive_switches: u16
}

impl State {
    fn new(pokemon: [Pokemon; 12], weather: Weather, terrain: Terrain, rng: &mut StdRng) -> State {
        let mut state = State {
            pokemon,
            min_pokemon_id: None,
            max_pokemon_id: None,
            weather,
            terrain,
            turn_number: 0,
            display_text: Vec::new(),
            children: Vec::new(),
            max_actions: Vec::new(),
            min_actions: Vec::new(),
            max_action_order: Vec::new(),
            min_action_order: Vec::new(),
            max_consecutive_switches: 0,
            min_consecutive_switches: 0
        };

        generate_actions(&mut state, rng);

        state
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
            min_pokemon_id: self.min_pokemon_id,
            max_pokemon_id: self.max_pokemon_id,
            weather: self.weather,
            terrain: self.terrain,
            turn_number: self.turn_number,
            display_text: Vec::new(),
            children: Vec::new(),
            max_actions: Vec::new(),
            min_actions: Vec::new(),
            max_action_order: Vec::new(),
            min_action_order: Vec::new(),
            max_consecutive_switches: self.max_consecutive_switches,
            min_consecutive_switches: self.min_consecutive_switches
        }
    }

    pub fn battle_end_check(&self) -> bool {
        self.pokemon[0..6].iter().all(|pokemon| pokemon.current_hp() == 0) || self.pokemon[6..12].iter().all(|pokemon| pokemon.current_hp() == 0)
    }

    /// Gets the specified child or generates it using this state's actions if it does not exist.
    ///
    /// Accesses children through the action orderings, giving the appearance that the child
    /// matrix is sorted by whichever actions are expected to produce the best outcome.
    fn get_or_gen_child(&mut self, i: usize, j: usize, rng: &mut StdRng) -> &mut State {
        let max_action_index = self.max_action_order[i];
        let min_action_index = self.min_action_order[j];
        let child_index = max_action_index * self.min_actions.len() + min_action_index;

        if self.children[child_index].is_none() {
            let mut child = self.copy_game_state();
            let max_action = &*self.max_actions[max_action_index];
            let min_action = &*self.min_actions[min_action_index];
            child.max_consecutive_switches = match max_action {
                Action::Switch { .. } => child.max_consecutive_switches + 1,
                _ => 0
            };
            child.min_consecutive_switches = match min_action {
                Action::Switch { .. } => child.min_consecutive_switches + 1,
                _ => 0
            };
            play_out_turn(&mut child, vec![max_action, min_action], rng);
            generate_actions(&mut child, rng);
            self.children[child_index] = Some(child);
        }

        self.children[child_index].as_mut().unwrap()
    }

    /// Removes the specified child from this state.
    ///
    /// Accesses children through the action orderings, giving the appearance that the child
    /// matrix is sorted by whichever actions are expected to produce the best outcome.
    fn remove_child(&mut self, i: usize, j: usize, rng: &mut StdRng) -> State {
        self.get_or_gen_child(i, j, rng);

        let max_action_index = self.max_action_order[i];
        let min_action_index = self.min_action_order[j];
        let child_index = max_action_index * self.min_actions.len() + min_action_index;
        self.children.remove(child_index).unwrap()
    }
}

/// Run a battle from an initial state; the maximizer and minimizer use game theory to choose their actions. All
/// state-space branching due to chance events during the course of each turn has been removed to reduce
/// computational complexity. Instead, one potential outcome is simply chosen at random (weighted appropriately),
/// so the agents behave as if they know the outcome ahead of time. Over many trials, the heuristic value should
/// average out to what one would obtain from a full state-space/probability tree search, but expect high variance
/// between individual trials. Returns a heuristic value between -1.0 and 1.0 signifying how well the maximizer did;
/// 0.0 would be a tie. The minimizer's value is its negation.
pub fn run_battle(minimizer: &TeamBuild, maximizer: &TeamBuild, rng: &mut StdRng) -> f64 {
    let mut state = State::new({
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
    }, Weather::default(), Terrain::default(), rng);

    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE BEGIN >>>>");
        state.print_display_text();
    }

    let mut nash_eq = iterative_deepening(&mut state, AI_LEVEL, rng);

    while !state.max_actions.is_empty() && !state.min_actions.is_empty() {
        let maximizer_choice = choose_weighted_index(&nash_eq.max_player_strategy, rng);
        let minimizer_choice = choose_weighted_index(&nash_eq.min_player_strategy, rng);

        let child = state.remove_child(maximizer_choice, minimizer_choice, rng);
        state = child;
        if cfg!(feature = "print-battle") { state.print_display_text(); }
        nash_eq = iterative_deepening(&mut state, AI_LEVEL, rng);
    }

    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE END >>>>");
    }

    nash_eq.expected_payoff
}

fn iterative_deepening(state: &mut State, max_recursions: u8, rng: &mut StdRng) -> ZeroSumNashEq {
    smab_search(state, -1.0, 1.0, AI_LEVEL, rng)
    //println!("Max actions: {:?}", state.max_actions);
    //println!("Min actions: {:?}", state.min_actions);
    //println!("Nash eq: {:?}", state.nash_eq);
    /*smab_search(state, -1.0, 1.0, 1, rng);
    for recursions in 2..=max_recursions {
        smab_search(state, -1.0, 1.0, recursions, rng);
    }*/
}

/// Simultaneous move alpha-beta search, implemented as a simplification of
/// [Alpha-Beta Pruning for Games with Simultaneous Moves](docs/Alpha-Beta_Pruning_for_Games_with_Simultaneous_Moves.pdf).
fn smab_search(state: &mut State, mut alpha: f64, mut beta: f64, recursions: u8, rng: &mut StdRng) -> ZeroSumNashEq {
    let m = state.max_actions.len();
    let n = state.min_actions.len();

    // If depth limit reached or either agent has no actions, stop search.
    if recursions < 1 || m == 0 || n == 0 {
        return ZeroSumNashEq {
            max_player_strategy: vec![1.0 / m as f64; m],
            min_player_strategy: vec![1.0 / n as f64; n],
            expected_payoff: (state.pokemon[6..12].iter().map(|pokemon| pokemon.current_hp() as f64 / pokemon.max_hp() as f64).sum::<f64>()
                - state.pokemon[0..6].iter().map(|pokemon| pokemon.current_hp() as f64 / pokemon.max_hp() as f64).sum::<f64>()) / 6.0
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
            expected_payoff: alpha
        };
    }

    if col_domination.iter().all(|b| *b) {
        return ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: beta
        };
    }

    let nash_eq = game_theory::calc_nash_eq(&payoff_matrix, &row_domination, &col_domination, 2.0);
    // TODO: Set state.max_action_order and state.min_action_order based on strategies
    nash_eq
}

fn generate_actions(state: &mut State, rng: &mut StdRng) {
    match state.min_pokemon_id.zip(state.max_pokemon_id) {
        None => agents_choose_pokemon_to_send_out(state),
        Some((max_pokemon_id, min_pokemon_id)) => { // Agents must choose actions for each Pokemon
            let max_actions = gen_actions_for_user(state, rng, max_pokemon_id);
            let min_actions = gen_actions_for_user(state, rng, min_pokemon_id);
            state.max_actions = max_actions;
            state.min_actions = min_actions;

            state.max_actions.sort_unstable_by(|act1, act2| action_comparator(act1, act2));
            state.min_actions.sort_unstable_by(|act1, act2| action_comparator(act1, act2));
        }
    }

    state.max_action_order = (0..state.max_actions.len()).collect();
    state.min_action_order = (0..state.min_actions.len()).collect();
    state.children = vec![None; state.max_actions.len() * state.min_actions.len()];
}

#[inline(never)]
fn agents_choose_pokemon_to_send_out(state: &mut State) {
    state.max_actions = match state.max_pokemon_id {
        None => (6..12)
            .filter(|id| state.pokemon_by_id(*id).current_hp() > 0)
            .map(|id| Box::new(Action::Switch {
                user_id: None,
                switching_in_id: id,
                target_position: FieldPosition::Max
            })).collect(),
        Some(_) => vec![Box::new(Action::Nop)]
    };

    state.min_actions = match state.min_pokemon_id {
        None => (0..6)
            .filter(|id| state.pokemon_by_id(*id).current_hp() > 0)
            .map(|id| Box::new(Action::Switch {
                user_id: None,
                switching_in_id: id,
                target_position: FieldPosition::Min
            })).collect(),
        Some(_) => vec![Box::new(Action::Nop)]
    };
}

#[inline(never)]
fn gen_actions_for_user(state: &mut State, rng: &mut StdRng, user_id: u8) -> Vec<Box<Action>> {
    let mut actions: Vec<Box<Action>> = Vec::with_capacity(9);

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
    for move_index in 0..user.known_moves().len() as u8 {
        if user.can_choose_move(Some(move_index)) {
            let move_ = user.known_move(move_index).move_();
            actions.push(Box::new(Action::Move {
                user_id,
                move_,
                move_index: Some(move_index),
                target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                    .filter(|field_pos| Move::targeting(move_).can_hit(user.field_position().unwrap(), *field_pos)).collect(),
            }));
        }
    }

    // TODO: Can Struggle be used if switch actions are available?
    if actions.is_empty() {
        let struggle = Move::id_by_name("Struggle").unwrap();
        actions.push(Box::new(Action::Move {
            user_id,
            move_: struggle,
            move_index: None,
            target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                .filter(|field_pos| Move::targeting(struggle).can_hit(user.field_position().unwrap(), *field_pos)).collect(),
        }));
    }

    if (user_id < 6 && state.min_consecutive_switches < 2) || (user_id >= 6 && state.max_consecutive_switches < 2) {
        for team_member_id in if user_id < 6 { 0..6 } else { 6..12 } {
            let team_member = state.pokemon_by_id(team_member_id);
            if team_member.current_hp() > 0 && team_member.field_position() == None && team_member.known_moves().iter().map(|known_move| known_move.pp).sum::<u8>() > 0 {
                actions.push(Box::new(Action::Switch {
                    user_id: Some(user_id),
                    switching_in_id: team_member_id as u8,
                    target_position: state.pokemon_by_id(user_id).field_position().unwrap()
                }));
            }
        }
    }

    actions
}

/*
fn generate_immediate_children(state: &mut State, rng: &mut StdRng) {
    match state.min_pokemon_id.zip(state.max_pokemon_id) {
        None => { // Agent(s) must choose Pokemon to send out
            match state.min_pokemon_id.xor(state.max_pokemon_id) {
                Some(id) => {
                    if id >= 6 { // Only minimizer must choose
                        let choices: Vec<u8> = (0..6)
                            .filter(|id| state.pokemon[*id as usize].current_hp() > 0)
                            .collect();
                        for choice in &choices {
                            let mut child = state.copy_game_state();
                            pokemon::add_to_field(&mut child, *choice, FieldPosition::Min);
                            state.children.push(child);
                        }
                        state.num_maximizer_actions = 1;
                        state.num_minimizer_actions = choices.len();
                    } else { // Only maximizer must choose
                        let choices: Vec<u8> = (6..12)
                            .filter(|id| state.pokemon[*id as usize].current_hp() > 0)
                            .collect();
                        for choice in &choices {
                            let mut child = state.copy_game_state();
                            pokemon::add_to_field(&mut child, *choice, FieldPosition::Max);
                            state.children.push(child);
                        }
                        state.num_maximizer_actions = choices.len();
                        state.num_minimizer_actions = 1;
                    }
                },
                None => { // Both agents must choose
                    let minimizer_choices: Vec<_> = (0..6)
                        .filter(|id| state.pokemon[*id as usize].current_hp() > 0)
                        .collect();

                    let maximizer_choices: Vec<_> = (6..12)
                        .filter(|id| state.pokemon[*id as usize].current_hp() > 0)
                        .collect();

                    for maximizer_choice in &maximizer_choices {
                        for minimizer_choice in &minimizer_choices {
                            let mut child = state.copy_game_state();
                            let battle_ended = pokemon::add_to_field(&mut child, *minimizer_choice, FieldPosition::Min);
                            if !battle_ended {
                                pokemon::add_to_field(&mut child, *maximizer_choice, FieldPosition::Max);
                            }
                            state.children.push(child);
                        }
                    }
                    state.num_maximizer_actions = maximizer_choices.len();
                    state.num_minimizer_actions = minimizer_choices.len();
                }
            }
        },
        Some((min_pokemon_id, max_pokemon_id)) => { // Agents must choose actions for each Pokemon
            let mut generate_actions = |user_id: u8| -> Vec<Action> {
                let mut actions: Vec<Action> = Vec::with_capacity(9);

                if let Some(next_move_action) = state.pokemon_by_id(user_id).next_move_action.clone() { // TODO: Is this actually what should happen?
                    if next_move_action.can_be_performed(state, rng) {
                        actions.push(next_move_action);
                        state.pokemon_by_id_mut(user_id).next_move_action = None;
                        return actions;
                    } else {
                        state.pokemon_by_id_mut(user_id).next_move_action = None;
                    }
                }

                let user = &state.pokemon[user_id as usize];
                for move_index in 0..user.known_moves().len() as u8 {
                    if user.can_choose_move(Some(move_index)) {
                        let move_ = user.known_move(move_index).move_();
                        actions.push(Action::Move {
                            user_id,
                            move_,
                            move_index: Some(move_index),
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

                for team_member_id in if user_id < 6 { 0..6 } else { 6..12 } {
                    let team_member = state.pokemon_by_id(team_member_id);
                    if team_member.current_hp() > 0 && team_member.field_position() == None && team_member.known_moves().iter().map(|known_move| known_move.pp).sum::<u8>() > 0 {
                        actions.push(Action::Switch {
                            user_id,
                            switching_in_id: team_member_id as u8
                        });
                    }
                }

                actions
            };

            let mut min_actions = generate_actions(min_pokemon_id);
            let mut max_actions = generate_actions(max_pokemon_id);

            min_actions.sort_unstable_by(|act1, act2| action_comparator(act1, act2, state));
            max_actions.sort_unstable_by(|act1, act2| action_comparator(act1, act2, state));

            for max_action in &max_actions {
                for min_action in &min_actions {
                    let mut child = state.copy_game_state();
                    play_out_turn(&mut child, vec![min_action, max_action], rng);
                    state.children.push(child);
                }
            }

            state.num_maximizer_actions = max_actions.len();
            state.num_minimizer_actions = min_actions.len();
        }
    }
}*/

// TODO: Make better; order actions so that pruning is most likely to occur.
#[inline(never)]
fn action_comparator(act1: &Action, act2: &Action) -> Ordering {
    match act1 {
        Action::Nop => Ordering::Greater,
        Action::Switch { .. } => {
            match act2 {
                Action::Nop => Ordering::Less,
                Action::Switch { .. } => Ordering::Equal,
                Action::Move { .. } => Ordering::Greater
            }
        },
        Action::Move {user_id: _, move_: act1_move, move_index: _, target_positions: _} => {
            match act2 {
                Action::Move {user_id: _, move_: act2_move, move_index: _, target_positions: _} => {
                    match Move::category(*act1_move) {
                        MoveCategory::Status => {
                            match Move::category(*act2_move) {
                                MoveCategory::Status => Ordering::Equal,
                                _ => Ordering::Greater
                            }
                        },
                        _ => {
                            match Move::category(*act2_move) {
                                MoveCategory::Status => Ordering::Less,
                                _ => Ordering::Equal
                            }
                        }
                    }
                },
                _ => Ordering::Less
            }
        }
    }
}

// TODO: Pass actions directly without using queues
fn play_out_turn(state: &mut State, mut action_queue: Vec<&Action>, rng: &mut StdRng) {
    let turn_number = state.turn_number;
    if cfg!(feature = "print-battle") {
        state.add_display_text(format!("---- Turn {} ----", turn_number));
    }

    for id in 0..12 {
        pokemon::increment_msa_counter(state, id);
    }

    if action_queue.len() == 2 && action_queue[1].outspeeds(state, action_queue[0], rng) {
        action_queue.swap(0, 1);
    }

    while !action_queue.is_empty() {
        let action = action_queue.remove(0);
        if action.can_be_performed(state, rng) && action.perform(state, &action_queue, rng) {
            return;
        }
    }

    // End of turn effects (order is randomized to avoid bias)
    let pokemon_ids = if rng.gen_bool(0.5) {
        vec![state.min_pokemon_id, state.max_pokemon_id]
    } else {
        vec![state.max_pokemon_id, state.min_pokemon_id]
    };

    for pokemon_id in pokemon_ids {
        if let Some(pokemon_id) = pokemon_id {
            match state.pokemon[pokemon_id as usize].major_status_ailment() {
                MajorStatusAilment::Poisoned => {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{} takes damage from poison!", state.pokemon[pokemon_id as usize]);
                        state.add_display_text(display_text);
                    }
                    if pokemon::apply_damage(state, pokemon_id, max(state.pokemon[pokemon_id as usize].max_hp() / 8, 1) as i16) {
                        return;
                    }
                },
                MajorStatusAilment::BadlyPoisoned => {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{} takes damage from poison!", state.pokemon[pokemon_id as usize]);
                        state.add_display_text(display_text);
                    }
                    let amount = {
                        let pokemon = state.pokemon_by_id(pokemon_id);
                        ((pokemon.msa_counter() + 1) * max(pokemon.max_hp() / 16, 1)) as i16
                    };
                    if pokemon::apply_damage(state, pokemon_id, amount) {
                        return;
                    }
                },
                _ => {}
            }

            if let Some(seeder_pos) = state.pokemon[pokemon_id as usize].seeded_by {
                let seeder_id = match seeder_pos {
                    FieldPosition::Min => state.min_pokemon_id,
                    FieldPosition::Max => state.max_pokemon_id
                };
                if let Some(seeder_id) = seeder_id {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{}'s seed drains energy from {}!", state.pokemon[seeder_id as usize], state.pokemon[pokemon_id as usize]);
                        state.add_display_text(display_text);
                    }
                    let transferred_hp = max(state.pokemon[pokemon_id as usize].max_hp() / 8, 1) as i16;
                    if pokemon::apply_damage(state, pokemon_id, transferred_hp) || pokemon::apply_damage(state, seeder_id, -transferred_hp) {
                        return;
                    }
                }
            }
        }
    }

    state.turn_number += 1;
}
