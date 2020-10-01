use crate::{choose_weighted_index, FieldPosition, Pokemon, Terrain, Weather, MajorStatusAilment};
use crate::move_::{MoveAction, Move};
use std::cmp::max;
use rand::Rng;
use std::f64::NAN;

const AI_LEVEL: u8 = 3;
const MAX_ACTIONS_PER_AGENT: usize = 9;

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
const fn run_battle(state: State) -> f64 {
    println!("<<<< BATTLE BEGIN >>>>");

    let mut state_space: StateSpace = StateSpace::new(state, MAX_ACTIONS_PER_AGENT.pow(2), AI_LEVEL as u32 + 1);
    let root_state = state_space.get(0).unwrap();

    root_state.print_display_text();
    generate_child_states(&mut state_space, 0, state_space.states.len());
    /*while !state.child_matrix.is_empty() {
        let nash_eq = state.heuristic_value();
        state = state.child_matrix
            .get_mut(choose_weighted_index(&nash_eq.max_player_strategy)).unwrap()
            .remove(choose_weighted_index(&nash_eq.min_player_strategy));
        state.print_display_text();
        state.generate_child_states(AI_LEVEL);
    }*/

    println!("<<<< BATTLE END >>>>");
    heuristic_value(&state_space, 0, state_space.states.len()).expected_payoff
}

/// Generates new child states to fill the state space.
fn generate_child_states(state_space: &mut StateSpace, parent_index: usize, parent_size: usize) {
    if let Some(parent) = state_space.get_mut(parent_index) {
        if !state_space.children(parent_index).is_empty() { return; } // Don't need to recompute children if they already exist

        let child_size = (parent_size - 1) / state_space.branching_factor;

        match parent.min_pokemon_id.zip(parent.max_pokemon_id) {
            None => { // Agent(s) must choose Pokemon to send out
                match parent.min_pokemon_id.xor(parent.max_pokemon_id) {
                    Some(id) => {
                        if id >= 6 { // Only minimizer must choose
                            let choices: Vec<u8> = (0..6).filter(|id| parent.pokemon_by_id(*id).current_hp > 0).collect();
                            for choice in choices {
                                let mut child_state = parent.copy_game_state();
                                child_state.pokemon_by_id_mut(choice).add_to_field(&mut child_state, FieldPosition::Min);
                                child_state.display_text.push(String::from(""));

                                let child_index = choice as usize * child_size + parent_index + 1;
                                if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                                state_space.states.insert(child_index, Some(child_state));
                                generate_child_states(state_space, child_index, child_size);
                            }
                        } else { // Only maximizer must choose
                            let choices: Vec<u8> = (6..12).filter(|id| parent.pokemon_by_id(*id).current_hp > 0).collect();
                            for choice in choices {
                                let mut child_state = parent.copy_game_state();
                                child_state.pokemon_by_id_mut(choice).add_to_field(&mut child_state, FieldPosition::Max);
                                child_state.display_text.push(String::from(""));

                                let child_index = (choice as usize - 6) * MAX_ACTIONS_PER_AGENT * child_size + parent_index + 1;
                                if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                                state_space.states.insert(child_index, Some(child_state));
                                generate_child_states(state_space, child_index, child_size);
                            }
                        }
                    },
                    None => { // Both agents must choose
                        let minimizer_choices: Vec<u8> = (0..6).filter(|id| parent.pokemon_by_id(*id).current_hp > 0).collect();
                        let maximizer_choices: Vec<u8> = (6..12).filter(|id| parent.pokemon_by_id(*id).current_hp > 0).collect();

                        for maximizer_choice in maximizer_choices {
                            for minimizer_choice in minimizer_choices {
                                let mut child_state = parent.copy_game_state();

                                let battle_ended = child_state.pokemon_by_id_mut(minimizer_choice).add_to_field(&mut child_state, FieldPosition::Min);
                                child_state.display_text.push(String::from(""));
                                if !battle_ended {
                                    child_state.pokemon_by_id_mut(maximizer_choice).add_to_field(&mut child_state, FieldPosition::Max);
                                    child_state.display_text.push(String::from(""));
                                }

                                let child_num = (maximizer_choice as usize - 6) * MAX_ACTIONS_PER_AGENT + minimizer_choice as usize;
                                let child_index = child_num * child_size + parent_index + 1;
                                if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                                state_space.states.insert(child_index, Some(child_state));
                                generate_child_states(state_space, child_index, child_size);
                            }
                        }
                    }
                }
            },
            Some((min_pokemon_id, max_pokemon_id)) => { // Agents must choose actions for each Pokemon
                // TODO: Rule out actions that are obviously not optimal to reduce search size
                let generate_move_actions = |user: &mut Pokemon| -> Vec<MoveAction> {
                    let min_pokemon = parent.pokemon_by_id(min_pokemon_id);
                    let max_pokemon = parent.pokemon_by_id(max_pokemon_id);
                    let mut user_actions: Vec<MoveAction> = vec![];

                    if let Some(next_move_action) = user.next_move_action.clone() { // TODO: Is this actually what should happen?
                        if next_move_action.can_be_performed(parent) {
                            user_actions.push(next_move_action);
                            user.next_move_action = None;
                            return user_actions;
                        } else {
                            user.next_move_action = None;
                        }
                    }

                    let mut add_move_action = |move_: &'static Move, move_index: Option<usize>| {
                        user_actions.push(MoveAction {
                            user_id: user.id,
                            move_,
                            move_index,
                            target_positions: [min_pokemon.field_position.unwrap(), max_pokemon.field_position.unwrap()].iter().copied()
                                .filter(|field_pos| move_.targeting.can_hit(user.field_position.unwrap(), *field_pos)).collect()
                        });
                    };

                    for move_index in 0..user.known_moves.len() {
                        if user.can_choose_move(Some(move_index)) {
                            add_move_action(user.known_moves.get(move_index).unwrap(), Some(move_index));
                        }
                    }

                    // TODO: Can Struggle be used if switch actions are available?
                    if user_actions.is_empty() {
                        add_move_action(unsafe { &crate::move_::STRUGGLE }, None);
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
                let mut minimizer_move_actions: Vec<MoveAction> = generate_move_actions(parent.pokemon_by_id_mut(min_pokemon_id));
                let mut maximizer_move_actions: Vec<MoveAction> = generate_move_actions(parent.pokemon_by_id_mut(max_pokemon_id));

                for maximizer_choice in 0..maximizer_move_actions.len() {
                    for minimizer_choice in 0..minimizer_move_actions.len() {
                        let mut child_state = parent.copy_game_state();
                        child_state.play_out_turn(vec![minimizer_move_actions.get(minimizer_choice).unwrap(), maximizer_move_actions.get(maximizer_choice).unwrap()]);

                        let child_num = (maximizer_choice as usize + 6) * MAX_ACTIONS_PER_AGENT + minimizer_choice as usize + 6;
                        let child_index = child_num * child_size + parent_index + 1;
                        if state_space.states.remove(child_index).is_some() { panic!("Removed an existing child") };
                        state_space.states.insert(child_index, Some(child_state));
                        generate_child_states(state_space, child_index, child_size);
                    }
                }
            }
        }
    }
}

/// Recursively computes the heuristic value of a state.
fn heuristic_value(state_space: &StateSpace, parent_index: usize, parent_size: usize) -> ZeroSumNashEq {
    let parent = state_space.get(parent_index).unwrap();
    let children = state_space.children(parent_index);
    if children.is_empty() {
        ZeroSumNashEq {
            max_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
            min_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
            expected_payoff: (parent.pokemon_by_id[6..12].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()
                - parent.pokemon_by_id[0..6].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()) / 6.0
        }
    } else {
        let child_size = (parent_size - 1) / state_space.branching_factor;
        let mut payoff_matrix = [[NAN; MAX_ACTIONS_PER_AGENT]; MAX_ACTIONS_PER_AGENT];

        for maximizer_choice in 0..MAX_ACTIONS_PER_AGENT {
            for minimizer_choice in 0..MAX_ACTIONS_PER_AGENT {
                let child_num = maximizer_choice * MAX_ACTIONS_PER_AGENT + minimizer_choice;
                let child_index = child_num * child_size + parent_index + 1;
                payoff_matrix[maximizer_choice][minimizer_choice] = heuristic_value(state_space, child_index, child_size).expected_payoff;
            }
        }

        State::calc_nash_eq(payoff_matrix)
    }
}

/// A full (also complete and balanced) n-ary tree in prefix order.
struct StateSpace {
    states: Vec<Option<State>>,
    branching_factor: usize,
    num_levels: u32
}

impl StateSpace {
    const fn new(root: State, branching_factor: usize, num_levels: u32) -> StateSpace {
        if num_levels == 0 { panic!(format!("Number of levels must be at least 1. Given: {}", num_levels)); }

        let mut states = vec![Some(root)];
        states.resize_with((0..num_levels).map(|depth| branching_factor.pow(depth)).sum(), || None);
        StateSpace {
            states,
            branching_factor,
            num_levels
        }
    }

    const fn get(&self, index: usize) -> Option<&State> {
        self.states.get(index).unwrap().as_ref()
    }

    const fn get_mut(&mut self, index: usize) -> Option<&mut State> {
        self.states.get_mut(index).unwrap().as_mut()
    }

    /// Indices of the direct children of the state at 'parent_index'.
    const fn child_indices(&self, parent_index: usize) -> Vec<usize> {
        let child_size = (self.depth_and_subtree_size(parent_index).1 - 1) / self.branching_factor;

        if child_size == 0 { return vec![]; }

        (0..self.branching_factor).map(|child_num| child_num * child_size + parent_index + 1).collect()
    }

    /// References to the direct children of the state at 'parent_index'.
    const fn children(&self, parent_index: usize) -> Vec<&State> {
        self.child_indices(parent_index).iter().filter_map(|index| self.states.get(*index).unwrap().as_ref()).collect()
    }

    /// Replaces the entire tree with the subtree rooted at 'index' and fills the gaps with None.
    fn prune_expand(&mut self, index: usize) {
        let (pruned_subtree_depth, pruned_subtree_size) = self.depth_and_subtree_size(index);
        self.states.truncate(index + pruned_subtree_size);
        for _ in 0..index { self.states.remove(0); }

        self._expand(0, self.states.len(), pruned_subtree_depth);
    }

    fn _expand(&mut self, subtree_start: usize, subtree_size: usize, num_levels_to_add: u32) {
        if num_levels_to_add > 0 {
            if subtree_size == 1 {
                for _ in 0..self.branching_factor {
                    self.states.insert(subtree_start + 1, None);
                    self._expand(subtree_start + 1, 1, num_levels_to_add - 1);
                }
            } else {
                let child_size = (subtree_size - 1) / self.branching_factor;
                for child_num in (0..self.branching_factor).rev() {
                    let child_subtree_start = child_num * child_size + 1;
                    self._expand(child_subtree_start, child_size, num_levels_to_add);
                }
            }
        }
    }

    /// The depth and size of a subtree rooted at 'index'. The root is at depth 0.
    const fn depth_and_subtree_size(&self, index: usize) -> (u32, usize) {
        self._depth_and_subtree_size(index, 0, self.states.len(), 0)
    }

    fn _depth_and_subtree_size(&self, index: usize, subtree_start: usize, subtree_size: usize, depth: u32) -> (u32, usize) {
        if index == subtree_start {
            return (depth, subtree_size);
        }

        let child_size = (subtree_size - 1) / self.branching_factor;

        for child_num in 0..self.branching_factor {
            let child_subtree_start = child_num * child_size + 1;
            if index < child_subtree_start + child_size {
                return self._depth_and_subtree_size(index, child_subtree_start, child_size, depth + 1);
            }
        }
        panic!("This should never happen.");
    }
}

/// Represents the entire game state of a battle.
#[derive(Debug)]
pub struct State {
    pokemon_by_id: [Pokemon; 12], // ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team
    pub min_pokemon_id: Option<u8>, // Pokemon of the minimizing team that is on the field
    pub max_pokemon_id: Option<u8>, // Pokemon of the maximizing team that is on the field
    pub weather: Weather,
    terrain: Terrain,
    turn_number: u32,
    pub display_text: Vec<String> // Battle print-out that is shown when this state is entered; useful for sanity checks
}

impl State {
    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    // TODO: Pass actions directly without using queues
    fn play_out_turn(&mut self, mut move_action_queue: Vec<&MoveAction>) {
        self.display_text.push(format!("---- Turn {} ----", self.turn_number));

        if move_action_queue.len() == 2 && move_action_queue.get(1).unwrap().outspeeds(self, move_action_queue.get(0).unwrap()) {
            move_action_queue.push(move_action_queue.remove(0));
        } else if move_action_queue.len() > 2 { panic!("'moveActionQueue' size is greater than 2."); }

        while !move_action_queue.is_empty() {
            let move_action = move_action_queue.remove(0);
            move_action.pre_move_stuff(self);
            if move_action.can_be_performed(self) && move_action.perform(self, &move_action_queue) {
                return;
            }
        }

        // TODO: Use seeded RNG
        // End of turn effects (order is randomized to avoid bias)
        if rand::thread_rng().gen_bool(0.5) {
            if let Some(min_pokemon_id) = self.min_pokemon_id {
                if self.end_of_turn_effects(self.pokemon_by_id_mut(min_pokemon_id)) { return; }
            }
            if let Some(max_pokemon_id) = self.max_pokemon_id {
                if self.end_of_turn_effects(self.pokemon_by_id_mut(max_pokemon_id)) { return; }
            }
        } else {
            if let Some(max_pokemon_id) = self.max_pokemon_id {
                if self.end_of_turn_effects(self.pokemon_by_id_mut(max_pokemon_id)) { return; }
            }
            if let Some(min_pokemon_id) = self.min_pokemon_id {
                if self.end_of_turn_effects(self.pokemon_by_id_mut(min_pokemon_id)) { return; }
            }
        }

        self.display_text.push(String::from(""));
        self.turn_number += 1;
    }

    fn end_of_turn_effects(&mut self, pokemon: &mut Pokemon) -> bool {
        if pokemon.major_status_ailment() == MajorStatusAilment::Poisoned {
            self.display_text.push(format!("{} takes damage from poison!", pokemon));
            if pokemon.apply_damage(self, max(pokemon.max_hp / 8, 1) as i16) {
                return true;
            }
        }

        if let Some(seeder_id) = pokemon.seeded_by {
            let seeder = self.pokemon_by_id_mut(seeder_id);
            self.display_text.push(format!("{}'s seed drains energy from {}!", seeder, pokemon));
            let transferred_hp = max(pokemon.max_hp / 8, 1) as i16;
            if pokemon.apply_damage(self, transferred_hp) { return true; }
            seeder.apply_damage(self, -transferred_hp);
        }
        false
    }

    /// Calculates the Nash equilibrium of a zero-sum payoff matrix. Rows or columns that are not
    /// in use should contain NAN.
    /// Algorithm follows https://www.math.ucla.edu/~tom/Game_Theory/mat.pdf, section 4.5.
    fn calc_nash_eq(payoff_matrix: [[f64; MAX_ACTIONS_PER_AGENT]; MAX_ACTIONS_PER_AGENT]) -> ZeroSumNashEq {
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

        for i in 0..m { tableau[i][n] = 1.0; }
        for j in 0..n { tableau[m][j] = -1.0; }

        // Row player's labels are positive
        let mut left_labels = [0; MAX_ACTIONS_PER_AGENT];
        for i in 0..m { left_labels[i] = i as i64 + 1; }

        // Column player's labels are negative
        let mut top_labels = [0; MAX_ACTIONS_PER_AGENT];
        for j in 0..n { top_labels[j] = -(j as i64 + 1); }

        let mut negative_remaining = true;
        while negative_remaining {
            let mut q = 0; // Column to pivot on
            for j in 1..n {
                if tableau[m][j] < tableau[m][q] { q = j; }
            }
            let mut p = 0; // Row to pivot on
            for possible_p in 1..m {
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
            for i in 0..(m + 1) {
                if i != p { tableau[i][q] /= -pivot; }
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
        State {
            pokemon_by_id: self.pokemon_by_id.clone(),
            min_pokemon_id: self.min_pokemon_id,
            max_pokemon_id: self.max_pokemon_id,
            weather: self.weather,
            terrain: self.terrain,
            turn_number: self.turn_number,
            display_text: vec![]
        }
    }

    pub fn pokemon_by_id(&self, id: u8) -> &Pokemon {
        &self.pokemon_by_id[id as usize]
    }

    pub fn pokemon_by_id_mut(&mut self, id: u8) -> &mut Pokemon {
        &mut self.pokemon_by_id[id as usize]
    }

    pub fn battle_end_check(&self) -> bool {
        self.pokemon_by_id[0..5].iter().all(|pokemon| pokemon.current_hp == 0) || self.pokemon_by_id[6..11].iter().all(|pokemon| pokemon.current_hp == 0)
    }
}

struct Matrix {
    elements: Vec<Vec<f64>>,
    num_rows: usize,
    num_cols: usize
}

impl Matrix {
    const fn from(elements: Vec<Vec<f64>>) -> Matrix {
        let num_rows = elements.len();
        if num_rows == 0 { panic!("Matrix must not be empty."); }
        let num_cols = elements.get(0).unwrap().len();
        if num_cols == 0 { panic!("Matrix must not be empty.") }

        Matrix {
            elements,
            num_rows,
            num_cols
        }
    }

    const fn get(&self, row: usize, col: usize) -> Option<f64> {
        if let Some(inner_vec) = self.elements.get(row) {
            return inner_vec.get(col).copied()
        }
        None
    }
}

struct ZeroSumNashEq {
    max_player_strategy: [f64; MAX_ACTIONS_PER_AGENT], // The maximizing player's strategy at equilibrium
    min_player_strategy: [f64; MAX_ACTIONS_PER_AGENT], // The minimizing player's strategy at equilibrium
    expected_payoff: f64, // The expected payoff for the maximizing player at equilibrium
}
