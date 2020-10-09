use std::cmp::max;

use rand::Rng;

use crate::{choose_weighted_index, FieldPosition, MajorStatusAilment, pokemon, Terrain, Weather};
use crate::move_::MoveActionV2;
use crate::pokemon::PokemonV2;

const AI_LEVEL: u8 = 4;
const MAX_ACTIONS_PER_AGENT: usize = 9;

#[derive(Debug)]
struct ZeroSumNashEq {
    max_player_strategy: [f64; MAX_ACTIONS_PER_AGENT],
    // The maximizing player's strategy at equilibrium
    min_player_strategy: [f64; MAX_ACTIONS_PER_AGENT],
    // The minimizing player's strategy at equilibrium
    expected_payoff: f64, // The expected payoff for the maximizing player at equilibrium
}

/// Represents the entire game state of a battle.
#[derive(Debug)]
pub struct StateV2 {
    pub pokemon: [Box<PokemonV2>; 12],
    // ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team
    pub min_pokemon_id: Option<u8>,
    // Pokemon of the minimizing team that is on the field
    pub max_pokemon_id: Option<u8>,
    // Pokemon of the maximizing team that is on the field
    pub weather: Weather,
    pub terrain: Terrain,
    pub turn_number: u16,
    pub display_text: Vec<String>,
    // Battle print-out that is shown when this state is entered; useful for sanity checks
    pub children: Vec<Box<StateV2>>,
    pub num_maximizer_actions: usize,
    pub num_minimizer_actions: usize,
}

impl StateV2 {
    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    /// Calculates the Nash equilibrium of a zero-sum payoff matrix.
    /// Algorithm follows https://www.math.ucla.edu/~tom/Game_Theory/mat.pdf, section 4.5.
    fn calc_nash_eq(payoff_matrix: Vec<f64>, m: usize, n: usize) -> ZeroSumNashEq {
        // Algorithm requires that all elements be positive, so ADDED_CONSTANT is added to ensure this.
        const ADDED_CONSTANT: f64 = 2.0;

        let mut tableau = [[0.0; MAX_ACTIONS_PER_AGENT + 1]; MAX_ACTIONS_PER_AGENT + 1];
        for i in 0..m {
            for j in 0..n {
                tableau[i][j] = payoff_matrix.get(i * n + j).unwrap() + ADDED_CONSTANT;
            }
        }

        for row in tableau.iter_mut().take(m) { row[n] = 1.0; }
        for j in 0..n { tableau[m][j] = -1.0; }

        // Row player's labels are positive
        let mut left_labels = [0; MAX_ACTIONS_PER_AGENT];
        for (i, label) in left_labels.iter_mut().enumerate().take(m) {
            *label = i as i64 + 1;
        }

        // Column player's labels are negative
        let mut top_labels = [0; MAX_ACTIONS_PER_AGENT];
        for (j, label) in top_labels.iter_mut().enumerate().take(n) {
            *label = -(j as i64 + 1);
        }

        let mut negative_remaining = true;
        while negative_remaining {
            let mut q = 0; // Column to pivot on
            for j in 1..n {
                if tableau[m][j] < tableau[m][q] { q = j; }
            }
            let mut p = 0; // Row to pivot on
            for possible_p in 0..m {
                if tableau[possible_p][q] > 1e-12 && (tableau[possible_p][n] / tableau[possible_p][q] < tableau[p][n] / tableau[p][q] || tableau[p][q] <= 1e-12) {
                    p = possible_p;
                }
            }

            // Pivot
            let pivot = tableau[p][q];
            for j in 0..(n + 1) {
                for i in 0..(m + 1) {
                    if i != p && j != q { tableau[i][j] -= tableau[p][j] * tableau[i][q] / pivot; }
                }
            }
            for j in 0..(n + 1) {
                if j != q { tableau[p][j] /= pivot; }
            }
            for (i, row) in tableau.iter_mut().enumerate().take(m + 1) {
                if i != p { row[q] /= -pivot; }
            }
            tableau[p][q] = 1.0 / pivot;

            // Exchange labels appropriately
            let temp = left_labels[p];
            left_labels[p] = top_labels[q];
            top_labels[q] = temp;

            negative_remaining = false;
            for j in 0..n {
                if tableau[m][j] < 0.0 {
                    negative_remaining = true;
                    break;
                }
            }
        }

        let mut max_player_strategy = [0.0; MAX_ACTIONS_PER_AGENT];
        let mut min_player_strategy = [0.0; MAX_ACTIONS_PER_AGENT];
        for j in 0..n {
            if top_labels[j] > 0 { // If it's one of row player's labels
                max_player_strategy[top_labels[j] as usize - 1] = tableau[m][j] / tableau[m][n];
            }
        }
        for i in 0..m {
            if left_labels[i] < 0 { // If it's one of column player's labels
                min_player_strategy[(-left_labels[i]) as usize - 1] = tableau[i][n] / tableau[m][n];
            }
        }

        ZeroSumNashEq {
            max_player_strategy,
            min_player_strategy,
            expected_payoff: 1.0 / tableau[m][n] - ADDED_CONSTANT,
        }
    }

    /// Copies only the game state into a new State instance; doesn't copy the child matrix or display text.
    fn copy_game_state(&self) -> StateV2 {
        StateV2 {
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

    pub fn pokemon_by_id_mut(&mut self, id: u8) -> &mut PokemonV2 {
        &mut self.pokemon[id as usize]
    }

    pub fn battle_end_check(&self) -> bool {
        self.pokemon[0..5].iter().all(|pokemon| pokemon.current_hp == 0) || self.pokemon[6..11].iter().all(|pokemon| pokemon.current_hp == 0)
    }
}

/**
 * Run a battle from an initial state; the maximizer and minimizer use game theory to choose their actions. All
 * state-space branching due to chance events during the course of each turn has been removed to reduce
 * computational complexity. Instead, one potential outcome is simply chosen at random (weighted appropriately),
 * so the agents behave as if they know the outcome ahead of time. Over many trials, the heuristic value should
 * average out to what one would obtain from a full state-space/probability tree search, but expect high variance
 * between individual trials.
 * @param state - the initial game state
 * @param aiLevel - how many turns ahead the agents look; basically equivalent to intelligence
 * @return A heuristic value between -1.0 and 1.0 signifying how well the maximizer did; 0.0 would be a tie. The
 * minimizer's value is just its negation.
 */
pub fn run_battle_v2(state: StateV2, print_battle: bool) -> f64 {
    if print_battle {
        println!("<<<< BATTLE BEGIN >>>>");
        state.print_display_text();
    }

    let mut state_box = Box::new(state);

    generate_child_states_v2(&mut state_box, AI_LEVEL);

    while !state_box.children.is_empty() {
        let nash_eq = heuristic_value_v2(&state_box);
        let minimizer_choice = choose_weighted_index(&nash_eq.min_player_strategy);
        let maximizer_choice = choose_weighted_index(&nash_eq.max_player_strategy);

        let child_index = maximizer_choice * state_box.num_minimizer_actions + minimizer_choice;
        let child_box = state_box.children.remove(child_index);
        state_box = child_box;
        if print_battle { state_box.print_display_text(); }
        generate_child_states_v2(&mut state_box, AI_LEVEL);
    }

    if print_battle { println!("<<<< BATTLE END >>>>"); }
    heuristic_value_v2(&state_box).expected_payoff
}

fn generate_child_states_v2(state_box: &mut Box<StateV2>, mut recursions: u8) {
    if recursions < 1 { return; }

    if state_box.children.is_empty() {
        match state_box.min_pokemon_id.zip(state_box.max_pokemon_id) {
            None => { // Agent(s) must choose Pokemon to send out
                match state_box.min_pokemon_id.xor(state_box.max_pokemon_id) {
                    Some(id) => {
                        if id >= 6 { // Only minimizer must choose
                            let choices: Vec<u8> = (0..6)
                                .filter(|id| state_box.pokemon[*id as usize].current_hp > 0)
                                .collect();
                            for choice in &choices {
                                let mut child_box = Box::new(state_box.copy_game_state());
                                pokemon::add_to_field_v2(&mut child_box, *choice, FieldPosition::Min);
                                state_box.children.push(child_box);
                            }
                            state_box.num_maximizer_actions = 1;
                            state_box.num_minimizer_actions = choices.len();
                        } else { // Only maximizer must choose
                            let choices: Vec<u8> = (6..12).filter(|id| state_box.pokemon[*id as usize].current_hp > 0).collect();
                            for choice in &choices {
                                let mut child_box = Box::new(state_box.copy_game_state());
                                pokemon::add_to_field_v2(&mut child_box, *choice, FieldPosition::Max);
                                state_box.children.push(child_box);
                            }
                            state_box.num_maximizer_actions = choices.len();
                            state_box.num_minimizer_actions = 1;
                        }
                    }
                    None => { // Both agents must choose
                        let minimizer_choices: Vec<_> = (0..6)
                            .filter(|id| state_box.pokemon[*id as usize].current_hp > 0)
                            .collect();

                        let maximizer_choices: Vec<_> = (6..12)
                            .filter(|id| state_box.pokemon[*id as usize].current_hp > 0)
                            .collect();

                        let (mut max_len, mut min_len) = (0, 0);

                        let mut min_flag = true;

                        for maximizer_choice in maximizer_choices {
                            for minimizer_choice in &minimizer_choices {
                                let mut child_box = Box::new(state_box.copy_game_state());
                                let battle_ended = pokemon::add_to_field_v2(&mut child_box,
                                                                            *minimizer_choice, FieldPosition::Min);
                                if !battle_ended {
                                    pokemon::add_to_field_v2(&mut child_box, maximizer_choice, FieldPosition::Max);
                                }
                                state_box.children.push(child_box);

                                min_len += min_flag as usize
                            }

                            min_flag = false;

                            max_len += 1
                        }
                        state_box.num_maximizer_actions = max_len;
                        state_box.num_minimizer_actions = min_len;
                    }
                }

                // This choice doesn't provide much information and its computational cost is relatively small, so do an extra recursion.
                recursions += 1;
            }
            Some((min_pokemon_id, max_pokemon_id)) => { // Agents must choose actions for each Pokemon
                // TODO: Rule out actions that are obviously not optimal to reduce search size
                let mut generate_move_actions = |user_id: u8| -> Vec<MoveActionV2> {
                    let mut user_actions: Vec<MoveActionV2> = Vec::with_capacity(4);

                    if let Some(next_move_action) = state_box.pokemon[user_id as usize].next_move_action.clone() { // TODO: Is this actually what should happen?
                        if next_move_action.can_be_performed(state_box) {
                            user_actions.push(next_move_action);
                            state_box.pokemon_by_id_mut(user_id).next_move_action = None;
                            return user_actions;
                        } else {
                            state_box.pokemon_by_id_mut(user_id).next_move_action = None;
                        }
                    }

                    let user = &state_box.pokemon[user_id as usize];
                    for move_index in 0..user.known_moves.len() {
                        if user.can_choose_move(Some(move_index)) {
                            let move_ = user.known_moves.get(move_index).unwrap().move_;
                            user_actions.push(MoveActionV2 {
                                user_id,
                                move_,
                                move_index: Some(move_index),
                                target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                                    .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect(),
                            });
                        }
                    }

                    // TODO: Can Struggle be used if switch actions are available?
                    if user_actions.is_empty() {
                        let move_ = unsafe { &crate::move_::STRUGGLE_V2 };
                        user_actions.push(MoveActionV2 {
                            user_id,
                            move_,
                            move_index: None,
                            target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                                .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect(),
                        });
                    }

                    // TODO: Exploring every switch action takes too long; maybe come up with some heuristic to decide when to switch?
                    /*
                    for(int teamMemberID : team.pokemonIDs) {
                        Pokemon teamMember = pokemonByID(teamMemberID);
                        if(teamMember.currentHP > 0 && teamMember.fieldPosition == null && Arrays.stream(teamMember.movePP).sum() > 0)
                            userActions.add(new SwitchAction(teamMemberID, user.id));
                    }*/

                    user_actions
                };

                // TODO: Generate switch actions separately
                let minimizer_move_actions: Vec<MoveActionV2> = generate_move_actions(min_pokemon_id);
                let maximizer_move_actions: Vec<MoveActionV2> = generate_move_actions(max_pokemon_id);

                for maximizer_choice in 0..maximizer_move_actions.len() {
                    for minimizer_choice in 0..minimizer_move_actions.len() {
                        let mut child_box = Box::new(state_box.copy_game_state());
                        play_out_turn_v2(&mut child_box, vec![minimizer_move_actions.get(minimizer_choice).unwrap(), maximizer_move_actions.get(maximizer_choice).unwrap()]);
                        state_box.children.push(child_box);
                    }
                }
                state_box.num_maximizer_actions = maximizer_move_actions.len();
                state_box.num_minimizer_actions = minimizer_move_actions.len();
            }
        }
    }

    // If children is still empty, battle has ended
    for child_box in &mut state_box.children {
        generate_child_states_v2(child_box, recursions - 1);
    }
}

/// Recursively computes the heuristic value of a state.
fn heuristic_value_v2(state_box: &Box<StateV2>) -> ZeroSumNashEq {
    if state_box.children.is_empty() {
        ZeroSumNashEq {
            max_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
            min_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
            expected_payoff: (state_box.pokemon[6..12].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()
                - state_box.pokemon[0..6].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()) / 6.0,
        }
    } else {
        let payoff_matrix = state_box.children.iter().map(|child_box| heuristic_value_v2
            (child_box).expected_payoff).collect();

        StateV2::calc_nash_eq(payoff_matrix, state_box.num_maximizer_actions, state_box
            .num_minimizer_actions)
    }
}

// TODO: Pass actions directly without using queues
fn play_out_turn_v2(state_box: &mut Box<StateV2>, mut move_action_queue: Vec<&MoveActionV2>) {
    let turn_number = state_box.turn_number;
    state_box.display_text.push(format!("---- Turn {} ----", turn_number));

    unsafe {
        if move_action_queue.len() == 2 && move_action_queue.get_unchecked(1).outspeeds(state_box, move_action_queue.get_unchecked(0)) {
            let first_move_action = move_action_queue.remove(0);
            move_action_queue.push(first_move_action);
        } else if move_action_queue.len() > 2 { panic!("'moveActionQueue' size is greater than 2."); }
    }

    while !move_action_queue.is_empty() {
        let move_action = move_action_queue.remove(0);
        move_action.pre_move_stuff(state_box);
        if move_action.can_be_performed(state_box) && move_action.perform(state_box, &move_action_queue) {
            return;
        }
    }

    // TODO: Use seeded RNG
    // End of turn effects (order is randomized to avoid bias)
    let pokemon_ids = if rand::thread_rng().gen_bool(0.5) {
        vec![state_box.min_pokemon_id, state_box.max_pokemon_id]
    } else {
        vec![state_box.max_pokemon_id, state_box.min_pokemon_id]
    };

    for pokemon_id in pokemon_ids {
        if let Some(pokemon_id) = pokemon_id {
            if state_box.pokemon[pokemon_id as usize].major_status_ailment() == MajorStatusAilment::Poisoned {
                let display_text = format!("{} takes damage from poison!", state_box.pokemon[pokemon_id as usize]);
                state_box.display_text.push(display_text);
                if pokemon::apply_damage_v2(state_box, pokemon_id, max(state_box.pokemon[pokemon_id as usize].max_hp / 8, 1) as i16) {
                    return;
                }
            }

            if let Some(seeder_id) = state_box.pokemon[pokemon_id as usize].seeded_by {
                let display_text = format!("{}'s seed drains energy from {}!", state_box.pokemon[pokemon_id as usize], state_box.pokemon[pokemon_id as usize]);
                state_box.display_text.push(display_text);
                let transferred_hp = max(state_box.pokemon[pokemon_id as usize].max_hp / 8, 1) as i16;
                if pokemon::apply_damage_v2(state_box, pokemon_id, transferred_hp) {
                    return;
                }
                pokemon::apply_damage_v2(state_box, seeder_id, -transferred_hp);
            }
        }
    }

    state_box.turn_number += 1;
}
