use crate::{choose_weighted_index, FieldPosition, Pokemon, Terrain, Weather, MajorStatusAilment};
use crate::move_::{MoveAction, Move};
use std::cmp::max;
use rand::Rng;

const MAX_ACTIONS_PER_AGENT: usize = 9;

/// Represents the entire game state of a battle, including metadata such as child states.
#[derive(Debug)]
pub struct State {
    // Game state
    pokemon_by_id: [Pokemon; 12], // ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team
    pub min_pokemon_id: Option<u8>, // Pokemon of the minimizing team that is on the field
    pub max_pokemon_id: Option<u8>, // Pokemon of the maximizing team that is on the field
    pub weather: Weather,
    terrain: Terrain,
    turn_number: u32,
    // TODO: display_text is cleared upon creating a child state anyways, so probably don't include it in State
    pub display_text: Vec<String>, // Battle print-out that is shown when this state is entered; useful for sanity checks
    child_matrix: Vec<Vec<State>> // An entry at [i][j] is the resultant state of the maximizer and minimizer performing actions i and j, respectively.
}

impl State {
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
    fn run_battle(mut state: State, ai_level: u8) -> f64 {
        println!("<<<< BATTLE BEGIN >>>>");

        state.print_display_text();
        state.generate_child_states(ai_level);
        while !state.child_matrix.is_empty() {
            let nash_eq = state.heuristic_value();
            state = state.child_matrix
                .get_mut(choose_weighted_index(&nash_eq.max_player_strategy)).unwrap()
                .remove(choose_weighted_index(&nash_eq.min_player_strategy));
            state.print_display_text();
            state.generate_child_states(ai_level);
        }

        println!("<<<< BATTLE END >>>>");
        state.heuristic_value().expected_payoff
    }

    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    // TODO: Pass actions directly without using queues
    fn play_out_turn(&mut self, mut move_action_queue: Vec<MoveAction>) {
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

    /// Generates new child states for a given number of future turns.
    fn generate_child_states(&mut self, mut recursions: u8) {
        if recursions < 1 { return; }

        if self.child_matrix.is_empty() {
            match self.min_pokemon_id.zip(self.max_pokemon_id) {
                None => { // Agent(s) must choose Pokemon to send out
                    match self.min_pokemon_id.xor(self.max_pokemon_id) {
                        Some(id) => {
                            if id < 6 { // Only minimizer must choose
                                let choices: Vec<u8> = (0..5).filter(|id| self.pokemon_by_id(*id).current_hp > 0).collect();
                                if choices.is_empty() {
                                    self.display_text.push(String::from(""));
                                    self.display_text.push(String::from("The maximizing team won!"));
                                } else {
                                    let mut inner_vec = vec![];
                                    self.child_matrix.push(inner_vec);
                                    for choice in choices {
                                        let mut child_state = self.copy_game_state();
                                        inner_vec.push(child_state);
                                        child_state.pokemon_by_id_mut(choice).add_to_field(&mut child_state, FieldPosition::Min);
                                        child_state.display_text.push(String::from(""));
                                    }
                                }
                            } else { // Only maximizer must choose
                                let choices: Vec<u8> = (6..11).filter(|id| self.pokemon_by_id(*id).current_hp > 0).collect();
                                if choices.is_empty() {
                                    self.display_text.push(String::from(""));
                                    self.display_text.push(String::from("The minimizing team won!"));
                                } else {
                                    for choice in choices {
                                        let mut child_state = self.copy_game_state();
                                        self.child_matrix.push(vec![child_state]);
                                        child_state.pokemon_by_id_mut(choice).add_to_field(&mut child_state, FieldPosition::Max);
                                        child_state.display_text.push(String::from(""));
                                    }
                                }
                            }
                        },
                        None => { // Both agents must choose
                            let minimizer_choices: Vec<u8> = (0..5).filter(|id| self.pokemon_by_id(*id).current_hp > 0).collect();
                            let maximizer_choices: Vec<u8> = (6..11).filter(|id| self.pokemon_by_id(*id).current_hp > 0).collect();

                            for maximizer_choice in maximizer_choices {
                                let mut inner_vec = vec![];
                                self.child_matrix.push(inner_vec);
                                for minimizer_choice in minimizer_choices {
                                    let mut child_state = self.copy_game_state();
                                    inner_vec.push(child_state);

                                    let battle_ended = child_state.pokemon_by_id_mut(minimizer_choice).add_to_field(&mut child_state, FieldPosition::Min);
                                    child_state.display_text.push(String::from(""));
                                    if !battle_ended {
                                        child_state.pokemon_by_id_mut(maximizer_choice).add_to_field(&mut child_state, FieldPosition::Max);
                                        child_state.display_text.push(String::from(""));
                                    }
                                }
                            }
                        }
                    }

                    // This choice doesn't provide much information and its computational cost is relatively small, so do an extra recursion.
                    recursions += 1;
                },
                Some((min_pokemon_id, max_pokemon_id)) => { // Agents must choose actions for each Pokemon
                    // TODO: Rule out actions that are obviously not optimal to reduce search size
                    let generate_move_actions = |user: &mut Pokemon| -> Vec<MoveAction> {
                        let min_pokemon = self.pokemon_by_id(min_pokemon_id);
                        let max_pokemon = self.pokemon_by_id(max_pokemon_id);
                        let mut user_actions: Vec<MoveAction> = vec![];

                        if let Some(next_move_action) = user.next_move_action.clone() { // TODO: Is this actually what should happen?
                            if next_move_action.can_be_performed(self) {
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
                    let mut minimizer_move_actions: Vec<MoveAction> = generate_move_actions(self.pokemon_by_id_mut(min_pokemon_id));
                    let mut maximizer_move_actions: Vec<MoveAction> = generate_move_actions(self.pokemon_by_id_mut(max_pokemon_id));

                    while !maximizer_move_actions.is_empty() {
                        let mut inner_vec = vec![];
                        self.child_matrix.push(inner_vec);
                        while !minimizer_move_actions.is_empty() {
                            let mut child_state = self.copy_game_state();
                            child_state.play_out_turn(vec![minimizer_move_actions.remove(0), maximizer_move_actions.remove(0)]);
                            inner_vec.push(child_state);
                        }
                    }
                }
            }
        }

        // If there are still no children by this point, the battle has ended

        for inner_vec in &mut self.child_matrix {
            for child in inner_vec {
                child.generate_child_states(recursions - 1);
            }
        }
    }

    /// Recursively computes the heuristic value of a state.
    const fn heuristic_value(&self) -> ZeroSumNashEq {
        if self.child_matrix.is_empty() {
            ZeroSumNashEq {
                max_player_strategy: vec![],
                min_player_strategy: vec![],
                expected_payoff: (self.pokemon_by_id[6..11].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()
                    - self.pokemon_by_id[0..5].iter().map(|pokemon| pokemon.current_hp as f64 / pokemon.max_hp as f64).sum::<f64>()) / 6.0
            }
        } else {
            let payoff_matrix: Vec<Vec<f64>> = self.child_matrix.iter().map(|inner_vec| {
                inner_vec.iter().map(|child| child.heuristic_value().expected_payoff).collect()
            }).collect();
            State::calc_nash_eq(&Matrix::from(payoff_matrix))
        }
    }

    /// Calculates the Nash equilibrium of a zero-sum payoff matrix.
    /// Algorithm follows https://www.math.ucla.edu/~tom/Game_Theory/mat.pdf, section 4.5.
    fn calc_nash_eq(payoff_matrix: &Matrix) -> ZeroSumNashEq {
        // Algorithm requires that all elements be positive, so ADDED_CONSTANT is added to ensure this.
        const ADDED_CONSTANT: f64 = 2.0;

        let m = payoff_matrix.num_rows;
        let n = payoff_matrix.num_cols;

        let mut tableau = [[0.0; MAX_ACTIONS_PER_AGENT + 1]; MAX_ACTIONS_PER_AGENT + 1];
        for i in 0..m {
            for j in 0..n {
                tableau[i][j] = payoff_matrix.get(i, j).unwrap() + ADDED_CONSTANT;
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

        let mut max_player_strategy_vec = max_player_strategy.to_vec();
        let mut min_player_strategy_vec = min_player_strategy.to_vec();
        max_player_strategy_vec.truncate(m);
        min_player_strategy_vec.truncate(n);
        ZeroSumNashEq {
            max_player_strategy: max_player_strategy_vec,
            min_player_strategy: min_player_strategy_vec,
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
            display_text: vec![],
            child_matrix: vec![]
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
    max_player_strategy: Vec<f64>, // The maximizing player's strategy at equilibrium
    min_player_strategy: Vec<f64>, // The minimizing player's strategy at equilibrium
    expected_payoff: f64, // The expected payoff for the maximizing player at equilibrium
}
