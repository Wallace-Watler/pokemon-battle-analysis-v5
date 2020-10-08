use crate::{Weather, Terrain, FieldPosition, pokemon, MajorStatusAilment, choose_weighted_index};
use std::f64::NAN;
use rand::Rng;
use std::cmp::max;
use crate::pokemon::Pokemon;
use crate::move_::MoveAction;
use coarse_prof::profile;

const AI_LEVEL: u8 = 2;
const MAX_ACTIONS_PER_AGENT: usize = 10;

#[derive(Debug)]
struct ZeroSumNashEq {
    max_player_strategy: [f64; MAX_ACTIONS_PER_AGENT], // The maximizing player's strategy at equilibrium
    min_player_strategy: [f64; MAX_ACTIONS_PER_AGENT], // The minimizing player's strategy at equilibrium
    expected_payoff: f64, // The expected payoff for the maximizing player at equilibrium
}

/// A full (also complete and balanced) n-ary tree in prefix order.
pub struct StateSpace {
    states: Vec<Option<State>>,
    branching_factor: usize
}

impl StateSpace {
    fn new(root: State, branching_factor: usize, num_levels: u32) -> StateSpace {
        if num_levels == 0 { panic!(format!("Number of levels must be at least 1. Given: {}", num_levels)); }

        let mut states = vec![Some(root)];
        states.resize_with((0..num_levels).map(|depth| branching_factor.pow(depth)).sum(), || None);
        StateSpace {
            states,
            branching_factor
        }
    }

    pub fn get(&self, state_id: usize) -> Option<&State> {
        profile!("get");
        self.states.get(state_id).unwrap().as_ref()
    }

    pub fn get_mut(&mut self, state_id: usize) -> Option<&mut State> {
        profile!("get_mut");
        self.states.get_mut(state_id).unwrap().as_mut()
    }

    /// Indices of the direct children of the state at `parent_index`.
    fn child_indices(&self, parent_index: usize) -> Vec<usize> {
        let child_size = (self.depth_and_subtree_size(parent_index).1 - 1) / self.branching_factor;

        if child_size == 0 { return vec![]; }

        (0..self.branching_factor).map(|child_num| child_num * child_size + parent_index + 1).collect()
    }

    /// Whether the state at `parent_index` has children.
    fn has_children(&self, parent_index: usize) -> bool {
        self.child_indices(parent_index).iter().any(|index| !self.states.get(*index).unwrap().is_none())
    }

    /// Replaces the entire tree with the subtree rooted at `index` and fills the gaps with None.
    fn prune_expand(&mut self, index: usize) {
        profile!("_expand");

        let (_, pruned_subtree_size) = self.depth_and_subtree_size(index);
        self.states.truncate(index + pruned_subtree_size);
        for _ in 0..index { self.states.remove(0); }

        self._expand(0, self.states.len());
    }

    #[inline(always)]
    fn _expand(&mut self, subtree_start: usize, subtree_size: usize) {
        self.states.reserve(self.branching_factor);

        if subtree_size == 1 {
            for _ in 0..self.branching_factor {
                self.states.insert(subtree_start + 1, None);
                //self._expand(subtree_start + 1, 1);
            }
        } else {
            let child_size = (subtree_size - 1) / self.branching_factor;
            for child_num in (0..self.branching_factor).rev() {
                let child_subtree_start = child_num * child_size + 1;
                self._expand(child_subtree_start, child_size);
            }
        }
    }

    /// The depth and size of a subtree rooted at `index`. The root is at depth 0.
    fn depth_and_subtree_size(&self, index: usize) -> (u32, usize) {
        self._depth_and_subtree_size(index, 0, self.states.len(), 0)
    }

    fn _depth_and_subtree_size(&self, index: usize, subtree_start: usize, subtree_size: usize, depth: u32) -> (u32, usize) {
        profile!("_depth_and_subtree_size");

        if index == subtree_start {
            return (depth, subtree_size);
        }

        let child_size = (subtree_size - 1) / self.branching_factor;

        let child_subtree_start = (0..self.branching_factor)
            .map(|child_num| child_num * child_size + 1 + subtree_start)
            .find(|child_subtree_start| index < child_subtree_start + child_size).expect("This should never happen.");

        self._depth_and_subtree_size(index, child_subtree_start, child_size, depth + 1)
    }
}

/// Represents the entire game state of a battle.
#[derive(Debug)]
pub struct State {
    pub pokemon: [Pokemon; 12], // ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team
    pub min_pokemon_id: Option<u8>, // Pokemon of the minimizing team that is on the field
    pub max_pokemon_id: Option<u8>, // Pokemon of the maximizing team that is on the field
    pub weather: Weather,
    pub terrain: Terrain,
    pub turn_number: u32,
    pub display_text: Vec<String> // Battle print-out that is shown when this state is entered; useful for sanity checks
}

impl State {
    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    /// Calculates the Nash equilibrium of a zero-sum payoff matrix. Rows or columns that are not
    /// in use should contain NAN.
    /// Algorithm follows https://www.math.ucla.edu/~tom/Game_Theory/mat.pdf, section 4.5.
    fn calc_nash_eq(payoff_matrix: [[f64; MAX_ACTIONS_PER_AGENT]; MAX_ACTIONS_PER_AGENT]) -> ZeroSumNashEq {
        profile!("calc_nash_eq");

        // Algorithm requires that all elements be positive, so ADDED_CONSTANT is added to ensure this.
        const ADDED_CONSTANT: f64 = 2.0;

        let m = MAX_ACTIONS_PER_AGENT;
        let n = MAX_ACTIONS_PER_AGENT;

        let mut tableau = [[0.0; MAX_ACTIONS_PER_AGENT + 1]; MAX_ACTIONS_PER_AGENT + 1];
        for i in 0..m {
            for j in 0..n {
                tableau[i][j] = payoff_matrix[i][j] + ADDED_CONSTANT;
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
        //for j in 0..n { top_labels[j] = -(j as i64 + 1); }
        for (j, label) in top_labels.iter_mut().enumerate().take(n) {
            *label = -(j as i64 + 1);
        }

        let mut negative_remaining = true;
        while negative_remaining {
            let mut q = MAX_ACTIONS_PER_AGENT + 1; // Column to pivot on
            for j in 0..n {
                if !tableau.iter().map(|row| row[j]).collect::<Vec<f64>>()[0..m].iter().all(|f| f.is_nan()) { // If the column is not all NAN
                    if q > MAX_ACTIONS_PER_AGENT || tableau[m][j] < tableau[m][q] { q = j; }
                }
            }
            let mut p = MAX_ACTIONS_PER_AGENT + 1; // Row to pivot on
            for possible_p in 0..m {
                if !tableau[possible_p][0..n].iter().all(|f| f.is_nan()) { // If the row is not all NAN
                    if p > MAX_ACTIONS_PER_AGENT
                        || (tableau[possible_p][q] > 1e-12 && (tableau[possible_p][n] / tableau[possible_p][q] < tableau[p][n] / tableau[p][q] || tableau[p][q] <= 1e-12)) {
                        p = possible_p;
                    }
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
            expected_payoff: 1.0 / tableau[m][n] - ADDED_CONSTANT
        }
    }

    /// Copies only the game state into a new State instance; doesn't copy the child matrix or display text.
    fn copy_game_state(&self) -> State {
        profile!("copy_game_state");

        State {
            pokemon: self.pokemon.clone(),
            min_pokemon_id: self.min_pokemon_id,
            max_pokemon_id: self.max_pokemon_id,
            weather: self.weather,
            terrain: self.terrain,
            turn_number: self.turn_number,
            display_text: vec![]
        }
    }

    pub fn pokemon_by_id(&self, id: u8) -> &Pokemon {
        profile!("pokemon_by_id");
        &self.pokemon[id as usize]
    }

    pub fn pokemon_by_id_mut(&mut self, id: u8) -> &mut Pokemon {
        profile!("pokemon_by_id_mut");
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
pub fn run_battle(state: State, print_battle: bool) -> f64 {
    profile!("run_battle");

    if print_battle {
        println!("<<<< BATTLE BEGIN >>>>");
        state.print_display_text();
    }

    let mut state_space: StateSpace = StateSpace::new(state, MAX_ACTIONS_PER_AGENT.pow(2), AI_LEVEL as u32 + 1);
    let state_space_size = state_space.states.len();
    generate_child_states(&mut state_space, 0, state_space_size);

    while state_space.has_children(0) {
        let nash_eq = heuristic_value(&state_space, 0, state_space_size);
        let minimizer_choice = choose_weighted_index(&nash_eq.min_player_strategy);
        let maximizer_choice = choose_weighted_index(&nash_eq.max_player_strategy);
        let child_num = maximizer_choice * MAX_ACTIONS_PER_AGENT + minimizer_choice;
        let child_index = child_num * (state_space_size - 1) / state_space.branching_factor + 1;
        state_space.prune_expand(child_index);
        if print_battle { state_space.get(0).unwrap().print_display_text(); }
        generate_child_states(&mut state_space, 0, state_space_size);
    }

    if print_battle { println!("<<<< BATTLE END >>>>"); }
    heuristic_value(&state_space, 0, state_space_size).expected_payoff
}

/// Generates new child states to fill the state space.
fn generate_child_states(state_space: &mut StateSpace, parent_id: usize, parent_size: usize) {
    profile!("generate_child_states");

    // Don't need to compute children if they already exist or if at maximum depth
    if state_space.depth_and_subtree_size(parent_id).1 == 1 || state_space.has_children(parent_id) {
        return;
    }

    let child_size = (parent_size - 1) / state_space.branching_factor;

    match state_space.get(parent_id).unwrap().min_pokemon_id.zip(state_space.get(parent_id).unwrap().max_pokemon_id) {
        None => { // Agent(s) must choose Pokemon to send out
            match state_space.get(parent_id).unwrap().min_pokemon_id.xor(state_space.get(parent_id).unwrap().max_pokemon_id) {
                Some(id) => {
                    if id >= 6 { // Only minimizer must choose
                        let choices: Vec<u8> = (0..6).filter(|id| state_space.get(parent_id).unwrap().pokemon_by_id(*id).current_hp > 0).collect();
                        for choice in choices {
                            let child_index = choice as usize * child_size + parent_id + 1;
                            if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                            state_space.states.insert(child_index, Some(state_space.get(parent_id).unwrap().copy_game_state()));

                            pokemon::add_to_field(state_space, child_index, choice, FieldPosition::Min);
                            generate_child_states(state_space, child_index, child_size);
                        }
                    } else { // Only maximizer must choose
                        let choices: Vec<u8> = (6..12).filter(|id| state_space.get(parent_id).unwrap().pokemon_by_id(*id).current_hp > 0).collect();
                        for choice in choices {
                            let child_index = (choice as usize - 6) * MAX_ACTIONS_PER_AGENT * child_size + parent_id + 1;
                            if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                            state_space.states.insert(child_index, Some(state_space.get(parent_id).unwrap().copy_game_state()));

                            pokemon::add_to_field(state_space, child_index, choice, FieldPosition::Max);
                            generate_child_states(state_space, child_index, child_size);
                        }
                    }
                },
                None => { // Both agents must choose
                    let minimizer_choices: Vec<u8> = (0..6).filter(|id| state_space.get(parent_id).unwrap().pokemon_by_id(*id).current_hp > 0).collect();
                    let maximizer_choices: Vec<u8> = (6..12).filter(|id| state_space.get(parent_id).unwrap().pokemon_by_id(*id).current_hp > 0).collect();

                    for maximizer_choice in maximizer_choices {
                        for minimizer_choice in &minimizer_choices {
                            let child_num = (maximizer_choice as usize - 6) * MAX_ACTIONS_PER_AGENT + *minimizer_choice as usize;
                            let child_index = child_num * child_size + parent_id + 1;
                            if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                            state_space.states.insert(child_index, Some(state_space.get(parent_id).unwrap().copy_game_state()));

                            let battle_ended = pokemon::add_to_field(state_space, child_index, *minimizer_choice, FieldPosition::Min);
                            if !battle_ended {
                                pokemon::add_to_field(state_space, child_index, maximizer_choice, FieldPosition::Max);
                            }

                            generate_child_states(state_space, child_index, child_size);
                        }
                    }
                }
            }
        },
        Some((min_pokemon_id, max_pokemon_id)) => { // Agents must choose actions for each Pokemon
            // TODO: Rule out actions that are obviously not optimal to reduce search size
            let mut generate_move_actions = |user_id: u8| -> Vec<MoveAction> {
                let mut user_actions: Vec<MoveAction> = vec![];

                if let Some(next_move_action) = state_space.get(parent_id).unwrap().pokemon_by_id(user_id).next_move_action.clone() { // TODO: Is this actually what should happen?
                    if next_move_action.can_be_performed(state_space, parent_id) {
                        user_actions.push(next_move_action);
                        state_space.get_mut(parent_id).unwrap().pokemon_by_id_mut(user_id).next_move_action = None;
                        return user_actions;
                    } else {
                        state_space.get_mut(parent_id).unwrap().pokemon_by_id_mut(user_id).next_move_action = None;
                    }
                }

                let user = state_space.get(parent_id).unwrap().pokemon_by_id(user_id);
                for move_index in 0..user.known_moves.len() {
                    if user.can_choose_move(Some(move_index)) {
                        let move_ = user.known_moves.get(move_index).unwrap().move_;
                        user_actions.push(MoveAction {
                            user_id,
                            move_,
                            move_index: Some(move_index),
                            target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                                .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect()
                        });
                    }
                }

                // TODO: Can Struggle be used if switch actions are available?
                if user_actions.is_empty() {
                    let move_ = unsafe { &crate::move_::STRUGGLE };
                    user_actions.push(MoveAction {
                        user_id,
                        move_,
                        move_index: None,
                        target_positions: [FieldPosition::Min, FieldPosition::Max].iter().copied()
                            .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect()
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
            let minimizer_move_actions: Vec<MoveAction> = generate_move_actions(min_pokemon_id);
            let maximizer_move_actions: Vec<MoveAction> = generate_move_actions(max_pokemon_id);

            for maximizer_choice in 0..maximizer_move_actions.len() {
                for minimizer_choice in 0..minimizer_move_actions.len() {
                    let child_num = (maximizer_choice as usize + 6) * MAX_ACTIONS_PER_AGENT + minimizer_choice as usize + 6;
                    let child_id = child_num * child_size + parent_id + 1;
                    if state_space.states.remove(child_id).is_some() { panic!("Removed an existing child") };
                    state_space.states.insert(child_id, Some(state_space.get(parent_id).unwrap().copy_game_state()));

                    play_out_turn(state_space, child_id, vec![minimizer_move_actions.get(minimizer_choice).unwrap(), maximizer_move_actions.get(maximizer_choice).unwrap()]);
                    generate_child_states(state_space, child_id, child_size);
                }
            }
        }
    }
}

/// Recursively computes the heuristic value of a state.
fn heuristic_value(state_space: &StateSpace, parent_index: usize, parent_size: usize) -> ZeroSumNashEq {
    profile!("heuristic_value");

    let parent = state_space.get(parent_index);
    match parent {
        Some(parent) => {
            if !state_space.has_children(parent_index) {
                ZeroSumNashEq {
                    max_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
                    min_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
                    expected_payoff: (parent.pokemon[6..12].iter().map(|pokemon: &Pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()
                        - parent.pokemon[0..6].iter().map(|pokemon: &Pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()) / 6.0
                }
            } else {
                let child_size = (parent_size - 1) / state_space.branching_factor;
                let mut payoff_matrix = [[NAN; MAX_ACTIONS_PER_AGENT]; MAX_ACTIONS_PER_AGENT];

                for (maximizer_choice, row) in payoff_matrix.iter_mut().enumerate().take(MAX_ACTIONS_PER_AGENT) {
                    for (minimizer_choice, entry) in row.iter_mut().enumerate().take(MAX_ACTIONS_PER_AGENT) {
                        let child_num = maximizer_choice * MAX_ACTIONS_PER_AGENT + minimizer_choice;
                        let child_index = child_num * child_size + parent_index + 1;
                        *entry = heuristic_value(state_space, child_index, child_size).expected_payoff;
                    }
                }

                State::calc_nash_eq(payoff_matrix)
            }
        },
        None => {
            ZeroSumNashEq {
                max_player_strategy: [NAN; MAX_ACTIONS_PER_AGENT],
                min_player_strategy: [NAN; MAX_ACTIONS_PER_AGENT],
                expected_payoff: NAN
            }
        }
    }
}

// TODO: Pass actions directly without using queues
fn play_out_turn(state_space: &mut StateSpace, state_id: usize, mut move_action_queue: Vec<&MoveAction>) {
    profile!("play_out_turn");

    let turn_number = state_space.get(state_id).unwrap().turn_number;
    state_space.get_mut(state_id).unwrap().display_text.push(format!("---- Turn {} ----", turn_number));

    unsafe {
        if move_action_queue.len() == 2 && move_action_queue.get_unchecked(1).outspeeds(state_space, state_id, move_action_queue.get_unchecked(0)) {
            let first_move_action = move_action_queue.remove(0);
            move_action_queue.push(first_move_action);
        } else if move_action_queue.len() > 2 { panic!("'moveActionQueue' size is greater than 2."); }
    }

    while !move_action_queue.is_empty() {
        let move_action = move_action_queue.remove(0);
        move_action.pre_move_stuff(state_space, state_id);
        if move_action.can_be_performed(state_space, state_id) && move_action.perform(state_space, state_id, &move_action_queue) {
            return;
        }
    }

    // TODO: Use seeded RNG
    // End of turn effects (order is randomized to avoid bias)
    let pokemon_ids = if rand::thread_rng().gen_bool(0.5) {
        vec![state_space.get(state_id).unwrap().min_pokemon_id, state_space.get(state_id).unwrap().max_pokemon_id]
    } else {
        vec![state_space.get(state_id).unwrap().max_pokemon_id, state_space.get(state_id).unwrap().min_pokemon_id]
    };

    for pokemon_id in pokemon_ids {
        profile!("End of turn effects");
        if let Some(pokemon_id) = pokemon_id {
            if state_space.get(state_id).unwrap().pokemon_by_id(pokemon_id).major_status_ailment() == MajorStatusAilment::Poisoned {
                let display_text = format!("{} takes damage from poison!", state_space.get(state_id).unwrap().pokemon_by_id(pokemon_id));
                state_space.get_mut(state_id).unwrap().display_text.push(display_text);
                if pokemon::apply_damage(state_space, state_id, pokemon_id, max(state_space.get(state_id).unwrap().pokemon_by_id(pokemon_id).max_hp / 8, 1) as i16) {
                    return;
                }
            }

            if let Some(seeder_id) = state_space.get(state_id).unwrap().pokemon_by_id(pokemon_id).seeded_by {
                let display_text = format!("{}'s seed drains energy from {}!", state_space.get(state_id).unwrap().pokemon_by_id(seeder_id), state_space.get(state_id).unwrap().pokemon_by_id(pokemon_id));
                state_space.get_mut(state_id).unwrap().display_text.push(display_text);
                let transferred_hp = max(state_space.get(state_id).unwrap().pokemon_by_id(pokemon_id).max_hp / 8, 1) as i16;
                if pokemon::apply_damage(state_space, state_id, pokemon_id, transferred_hp) {
                    return;
                }
                pokemon::apply_damage(state_space, state_id, seeder_id, -transferred_hp);
            }
        }
    }

    state_space.get_mut(state_id).unwrap().turn_number += 1;
}
