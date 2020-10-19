use std::cmp::{max, min, Ordering};

use rand::prelude::StdRng;
use rand::Rng;

use crate::{choose_weighted_index, FieldPosition, game_theory, MajorStatusAilment, pokemon, Terrain, Weather};
use crate::game_theory::{Matrix, ZeroSumNashEq};
use crate::pokemon::Pokemon;
use std::borrow::Borrow;
use crate::move_::Action;
use crate::move_;

pub const AI_LEVEL: u8 = 3;

/// Represents the entire game state of a battle.
#[derive(Debug)]
pub struct State {
    /// ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team.
    pub pokemon: [Box<Pokemon>; 12],
    /// Pokemon of the minimizing team that is on the field.
    pub min_pokemon_id: Option<u8>,
    /// Pokemon of the maximizing team that is on the field.
    pub max_pokemon_id: Option<u8>,
    pub weather: Weather,
    pub terrain: Terrain,
    pub turn_number: u16,
    /// Battle print-out that is shown when this state is entered; useful for sanity checks.
    pub display_text: Vec<String>,
    pub children: Vec<State>,
    pub num_maximizer_actions: usize,
    pub num_minimizer_actions: usize,
}

impl State {
    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    /// Copies only the game state into a new State instance; doesn't copy the child matrix or display text.
    fn copy_game_state(&self) -> State {
        State {
            pokemon: self.pokemon.clone(),
            min_pokemon_id: self.min_pokemon_id,
            max_pokemon_id: self.max_pokemon_id,
            weather: self.weather,
            terrain: self.terrain,
            turn_number: self.turn_number,
            display_text: Vec::new(),
            children: Vec::new(),
            num_maximizer_actions: 0,
            num_minimizer_actions: 0,
        }
    }

    pub fn battle_end_check(&self) -> bool {
        self.pokemon[0..5].iter().all(|pokemon| pokemon.current_hp == 0) || self.pokemon[6..11].iter().all(|pokemon| pokemon.current_hp == 0)
    }
}

/// Run a battle from an initial state; the maximizer and minimizer use game theory to choose their actions. All
/// state-space branching due to chance events during the course of each turn has been removed to reduce
/// computational complexity. Instead, one potential outcome is simply chosen at random (weighted appropriately),
/// so the agents behave as if they know the outcome ahead of time. Over many trials, the heuristic value should
/// average out to what one would obtain from a full state-space/probability tree search, but expect high variance
/// between individual trials. Returns a heuristic value between -1.0 and 1.0 signifying how well the maximizer did;
/// 0.0 would be a tie. The minimizer's value is its negation.
pub fn run_battle(mut state: State, rng: &mut StdRng) -> f64 {
    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE BEGIN >>>>");
        state.print_display_text();
    }

    build_state_tree(&mut state, AI_LEVEL, rng);

    while !state.children.is_empty() {
        let nash_eq = heuristic_value(&state);
        let minimizer_choice = choose_weighted_index(&nash_eq.min_player_strategy, rng);
        let maximizer_choice = choose_weighted_index(&nash_eq.max_player_strategy, rng);

        let child_index = maximizer_choice * state.num_minimizer_actions + minimizer_choice;
        let child_box = state.children.remove(child_index);
        state = child_box;
        if cfg!(feature = "print-battle") {
            state.print_display_text();
        }
        build_state_tree(&mut state, AI_LEVEL, rng);
    }

    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE END >>>>");
    }
    heuristic_value(&state).expected_payoff
}

/// Run a battle from an initial state; the maximizer and minimizer use game theory to choose their actions. All
/// state-space branching due to chance events during the course of each turn has been removed to reduce
/// computational complexity. Instead, one potential outcome is simply chosen at random (weighted appropriately),
/// so the agents behave as if they know the outcome ahead of time. Over many trials, the heuristic value should
/// average out to what one would obtain from a full state-space/probability tree search, but expect high variance
/// between individual trials. Returns a heuristic value between -1.0 and 1.0 signifying how well the maximizer did;
/// 0.0 would be a tie. The minimizer's value is its negation.
pub fn run_battle_smab(mut state: State, rng: &mut StdRng) -> f64 {
    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE BEGIN >>>>");
        state.print_display_text();
    }

    let mut nash_eq = smab_search(&mut state, -1.0, 1.0, AI_LEVEL, rng);

    while !state.children.is_empty() {
        let minimizer_choice = choose_weighted_index(&nash_eq.min_player_strategy, rng);
        let maximizer_choice = choose_weighted_index(&nash_eq.max_player_strategy, rng);

        let child_index = maximizer_choice * state.num_minimizer_actions + minimizer_choice;
        let child_box = state.children.remove(child_index);
        state = child_box;
        if cfg!(feature = "print-battle") { state.print_display_text(); }
        nash_eq = smab_search(&mut state, -1.0, 1.0, AI_LEVEL, rng);
    }

    if cfg!(feature = "print-battle") {
        println!("<<<< BATTLE END >>>>");
    }
    nash_eq.expected_payoff
}

fn build_state_tree(root: &mut State, recursions: u8, rng: &mut StdRng) {
    if recursions < 1 { return; }

    if root.children.is_empty() {
        generate_immediate_children(root, rng);
    }

    // If children is still empty, battle has ended
    for child in &mut root.children {
        build_state_tree(child, recursions - 1, rng);
    }
}

fn generate_immediate_children(state: &mut State, rng: &mut StdRng) {
    match state.min_pokemon_id.zip(state.max_pokemon_id) {
        None => { // Agent(s) must choose Pokemon to send out
            match state.min_pokemon_id.xor(state.max_pokemon_id) {
                Some(id) => {
                    if id >= 6 { // Only minimizer must choose
                        let choices: Vec<u8> = (0..6)
                            .filter(|id| state.pokemon[*id as usize].current_hp > 0)
                            .collect();
                        for choice in &choices {
                            let mut child = state.copy_game_state();
                            pokemon::add_to_field(&mut child, *choice, FieldPosition::Min);
                            state.children.push(child);
                        }
                        state.num_maximizer_actions = 1;
                        state.num_minimizer_actions = choices.len();
                    } else { // Only maximizer must choose
                        let choices: Vec<u8> = (6..12).filter(|id| state.pokemon[*id as usize].current_hp > 0).collect();
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
                        .filter(|id| state.pokemon[*id as usize].current_hp > 0)
                        .collect();

                    let maximizer_choices: Vec<_> = (6..12)
                        .filter(|id| state.pokemon[*id as usize].current_hp > 0)
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

                if let Some(next_move_action) = state.pokemon[user_id as usize].next_move_action.clone() { // TODO: Is this actually what should happen?
                    if next_move_action.can_be_performed(state, rng) {
                        actions.push(next_move_action);
                        state.pokemon[user_id as usize].next_move_action = None;
                        return actions;
                    } else {
                        state.pokemon[user_id as usize].next_move_action = None;
                    }
                }

                let user = &state.pokemon[user_id as usize];
                for move_index in 0..user.known_moves.len() {
                    if user.can_choose_move(Some(move_index)) {
                        let move_ = user.known_moves[move_index].move_;
                        actions.push(Action::Move {
                            user_id,
                            move_,
                            move_index: Some(move_index as u8),
                            target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                                .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect(),
                        });
                    }
                }

                // TODO: Can Struggle be used if switch actions are available?
                if actions.is_empty() {
                    let move_ = unsafe { &crate::move_::STRUGGLE };
                    actions.push(Action::Move {
                        user_id,
                        move_,
                        move_index: None,
                        target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                            .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect(),
                    });
                }

                for team_member_id in if user_id < 6 { 0..6 } else { 6..12 } {
                    let team_member: &Pokemon = state.pokemon[team_member_id].borrow();
                    if team_member.current_hp > 0 && team_member.field_position == None && team_member.known_moves.iter().map(|known_move| known_move.pp).sum::<u8>() > 0 {
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
}

// TODO: Make better
fn action_comparator(act1: &Action, act2: &Action, _played_in: &State) -> Ordering {
    match act1 {
        Action::Switch { .. } => {
            match act2 {
                Action::Switch { .. } => Ordering::Equal,
                Action::Move { .. } => Ordering::Greater
            }
        },
        Action::Move {user_id: _, move_: act1_move_, move_index: _, target_positions: _} => {
            match act2 {
                Action::Switch { .. } => Ordering::Less,
                Action::Move {user_id: _, move_: act2_move_, move_index: _, target_positions: _} => {
                    unsafe {
                        if *act1_move_ == &move_::TACKLE && *act2_move_ == &move_::TACKLE { return Ordering::Equal; }
                        if *act1_move_ == &move_::TACKLE && *act2_move_ != &move_::TACKLE { return Ordering::Less; }
                        if *act1_move_ != &move_::TACKLE && *act2_move_ == &move_::TACKLE { return Ordering::Greater; }
                        if *act1_move_ == &move_::GROWL && *act2_move_ == &move_::GROWL { return Ordering::Equal; }
                        if *act1_move_ == &move_::GROWL && *act2_move_ != &move_::GROWL { return Ordering::Less; }
                        if *act1_move_ != &move_::GROWL && *act2_move_ == &move_::GROWL { return Ordering::Greater; }
                        if *act1_move_ == &move_::VINE_WHIP && *act2_move_ == &move_::VINE_WHIP { return Ordering::Equal; }
                        if *act1_move_ == &move_::VINE_WHIP && *act2_move_ != &move_::VINE_WHIP { return Ordering::Less; }
                        if *act1_move_ != &move_::VINE_WHIP && *act2_move_ == &move_::VINE_WHIP { return Ordering::Greater; }
                        Ordering::Equal
                    }
                }
            }
        }
    }
}

/// Recursively computes the heuristic value of a state.
fn heuristic_value(state_box: &State) -> ZeroSumNashEq {
    if state_box.children.is_empty() {
        ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: (state_box.pokemon[6..12].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()
                - state_box.pokemon[0..6].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()) / 6.0,
        }
    } else {
        let payoff_matrix = Matrix::from(
            state_box.children.iter().map(|child_box| heuristic_value(child_box).expected_payoff).collect(),
            state_box.num_maximizer_actions,
            state_box.num_minimizer_actions);
        game_theory::calc_nash_eq(&payoff_matrix, &vec![false; payoff_matrix.num_rows()], &vec![false; payoff_matrix.num_cols()], 2.0)
    }
}

/// Simultaneous move alpha-beta search, implemented as a simplification of
/// [Alpha-Beta Pruning for Games with Simultaneous Moves](docs/Alpha-Beta_Pruning_for_Games_with_Simultaneous_Moves.pdf).
// TODO: Order moves/children so that pruning is most likely to occur
fn smab_search(state: &mut State, mut alpha: f64, mut beta: f64, recursions: u8, rng: &mut StdRng) -> ZeroSumNashEq {
    if recursions < 1 {
        return ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: (state.pokemon[6..12].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()
                - state.pokemon[0..6].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()) / 6.0,
        };
    }

    if state.children.is_empty() {
        generate_immediate_children(state, rng);
        if state.children.is_empty() { // If children is still empty, battle has ended.
            return smab_search(state, alpha, beta, 0, rng);
        }
    }

    let m = state.num_maximizer_actions;
    let n = state.num_minimizer_actions;

    let mut payoff_matrix = Matrix::of(0.0, m, n);
    let mut row_domination = vec![false; m];
    let mut col_domination = vec![false; n];
    let mut row_mins = vec![1.0; m];
    let mut col_maxes = vec![-1.0; n];

    let mut explore_child = |i: usize, j: usize| {
        if !row_domination[i] && !col_domination[j] {
            let child_value = smab_search(&mut state.children[i * n + j], alpha, beta, recursions - 1, rng).expected_payoff;
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
        ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: alpha
        }
    } else if col_domination.iter().all(|b| *b) {
        ZeroSumNashEq {
            max_player_strategy: Vec::new(),
            min_player_strategy: Vec::new(),
            expected_payoff: beta
        }
    } else {
        game_theory::calc_nash_eq(&payoff_matrix, &row_domination, &col_domination, 2.0)
    }
}

// TODO: Pass actions directly without using queues
fn play_out_turn(state: &mut State, mut action_queue: Vec<&Action>, rng: &mut StdRng) {
    let turn_number = state.turn_number;
    if cfg!(feature = "print-battle") {
        state.display_text.push(format!("---- Turn {} ----", turn_number));
    }

    if action_queue.len() == 2 && action_queue[1].outspeeds(state, action_queue[0], rng) {
        action_queue.swap(0, 1);
    }

    while !action_queue.is_empty() {
        let action = action_queue.remove(0);
        action.pre_action_stuff(state);
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
            if state.pokemon[pokemon_id as usize].major_status_ailment() == MajorStatusAilment::Poisoned {
                if cfg!(feature = "print-battle") {
                    let display_text = format!("{} takes damage from poison!", state.pokemon[pokemon_id as usize]);
                    state.display_text.push(display_text);
                }
                if pokemon::apply_damage(state, pokemon_id, max(state.pokemon[pokemon_id as usize].max_hp / 8, 1) as i16) {
                    return;
                }
            }

            if let Some(seeder_id) = state.pokemon[pokemon_id as usize].seeded_by {
                if cfg!(feature = "print-battle") {
                    let display_text = format!("{}'s seed drains energy from {}!", state.pokemon[pokemon_id as usize], state.pokemon[pokemon_id as usize]);
                    state.display_text.push(display_text);
                }
                let transferred_hp = max(state.pokemon[pokemon_id as usize].max_hp / 8, 1) as i16;
                if pokemon::apply_damage(state, pokemon_id, transferred_hp) {
                    return;
                }
                pokemon::apply_damage(state, seeder_id, -transferred_hp);
            }
        }
    }

    state.turn_number += 1;
}
