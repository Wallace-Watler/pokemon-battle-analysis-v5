use crate::{choose_weighted_index, FieldPosition, Pokemon, Terrain, Weather, MajorStatusAilment};
use crate::move_::{MoveAction, Move};
use std::cmp::max;
use rand::Rng;

const MAX_ACTIONS_PER_AGENT: usize = 9;
const ADDED_CONSTANT: f64 = 2.0; // Used in calculating Nash equilibria

// TODO: Store heuristic value and don't store payoff matrix
/// Represents the entire game state of a battle, including metadata such as child states.
#[derive(Debug)]
pub struct State {
    // Game state
    pokemon_by_id: [Pokemon; 12], // ID is the index
    minimizing_team: [u8; 6], // IDs of the Pokemon on the minimizing team
    maximizing_team: [u8; 6], // IDs of the Pokemon on the maximizing team
    pub min_pokemon_id: Option<u8>, // Pokemon of the minimizing team that is on the field
    pub max_pokemon_id: Option<u8>, // Pokemon of the maximizing team that is on the field
    pub weather: Weather,
    terrain: Terrain,
    turn_number: u32,
    pub display_text: Vec<String>, // Battle print-out that is shown when this state is entered; useful for sanity checks
    battle_ended: bool,
    //public final Random rand; // RNG used during battle

    // Metadata
    max_player_num_actions: u8,
    min_player_num_actions: u8,
    max_player_strategy: [f64; MAX_ACTIONS_PER_AGENT], // The maximizing player's strategy at equilibrium
    min_player_strategy: [f64; MAX_ACTIONS_PER_AGENT], // The minimizing player's strategy at equilibrium
    expected_payoff: f64, // The expected payoff for the maximizing player at equilibrium
    child_matrix: Vec<Vec<State>>, // An entry at [i][j] is the resultant state of the maximizer and minimizer performing actions i and j, respectively.
    payoff_matrix: [[f64; MAX_ACTIONS_PER_AGENT]; MAX_ACTIONS_PER_AGENT], // An entry at [i][j] is the maximizer's expected payoff given the maximizer and minimizer perform actions i and j, respectively.
}

impl State {
    // TODO: Consume the passed in State, return the final state (as well as heuristic value stored within that state)
    //   - Point is to discard the parent states as soon as possible, otherwise they're dead memory
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
    fn run_battle(mut state: &mut State, ai_level: u8) -> f64 {
        println!("<<<< BATTLE BEGIN >>>>");

        state.print_display_text();
        while !state.battle_ended {
            state.heuristic_value(ai_level);
            let child_state = state.child_matrix
                .get_mut(choose_weighted_index(&state.max_player_strategy)).unwrap()
                .get_mut(choose_weighted_index(&state.min_player_strategy)).unwrap();

            state = child_state;
            state.print_display_text();
        }

        println!("<<<< BATTLE END >>>>");
        state.heuristic_value(0)
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

    /// Computes the heuristic value of a state, generating new states as necessary.
    fn heuristic_value(&mut self, mut recursions: u8) -> f64 {
        if recursions < 1 || self.battle_ended {
            let team_value = |team: [u8; 6]| -> f64 {
                team.iter()
                    .map(|id| self.pokemon_by_id_mut(*id))
                    .map(|pokemon| pokemon.current_hp as f64 / (pokemon.max_hp as f64 * self.maximizing_team.len() as f64))
                    .sum()
            };
            return team_value(self.maximizing_team) - team_value(self.minimizing_team);
        }

        // Generates new states and recomputes the heuristic value of previously explored states.
        if self.child_matrix.is_empty() {
            match self.min_pokemon_id.zip(self.max_pokemon_id) {
                None => { // Agent(s) must choose Pokemon to send out
                    let mut minimizer_choices: Vec<u8> = vec![];
                    let mut maximizer_choices: Vec<u8> = vec![];

                    if self.min_pokemon_id == None {
                        for id in &self.minimizing_team {
                            if self.pokemon_by_id_mut(*id).current_hp > 0 { minimizer_choices.push(*id); }
                        }
                    }
                    if self.max_pokemon_id == None {
                        for id in &self.maximizing_team {
                            if self.pokemon_by_id_mut(*id).current_hp > 0 { maximizer_choices.push(*id); }
                        }
                    }

                    // TODO: Try setting num of choices directly equal to choices.len() (there shouldn't ever be 0 choices...? But if there is, don't do anything.)
                    let num_minimizer_choices = max(minimizer_choices.len(), 1);
                    let num_maximizer_choices = max(maximizer_choices.len(), 1);

                    for maximizer_choice_index in 0..num_maximizer_choices {
                        let mut inner_vec = vec![];
                        self.child_matrix.push(inner_vec);
                        for minimizer_choice_index in 0..num_minimizer_choices {
                            let mut child_state: State = self.copy_game_state();
                            child_state.display_text.clear();
                            inner_vec.push(child_state);

                            if !minimizer_choices.is_empty() {
                                let battle_ended = child_state
                                    .pokemon_by_id_mut(*minimizer_choices.get(minimizer_choice_index).unwrap())
                                    .add_to_field(&mut child_state, FieldPosition::Min);
                                child_state.display_text.push(String::from(""));
                                if battle_ended { break; }
                            }
                            if !maximizer_choices.is_empty() {
                                child_state
                                    .pokemon_by_id_mut(*maximizer_choices.get(maximizer_choice_index).unwrap())
                                    .add_to_field(&mut child_state, FieldPosition::Max);
                                child_state.display_text.push(String::from(""));
                            }
                        }
                    }

                    // This choice doesn't provide much information and its computational cost is relatively small, so do an extra recursion.
                    recursions += 1;
                },
                Some((min_pokemon_id, max_pokemon_id)) => { // Agents must choose actions for each Pokemon
                    // TODO: Rule out actions that are obviously not optimal to reduce state space size
                    let generate_move_actions = |user: &mut Pokemon, team: [u8; 6]| -> Vec<MoveAction> {
                        let min_pokemon = self.pokemon_by_id_mut(min_pokemon_id);
                        let max_pokemon = self.pokemon_by_id_mut(max_pokemon_id);
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

                        let add_move_action = |move_: &'static Move, move_index: Option<usize>| {
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

                        if user_actions.is_empty() { add_move_action(unsafe { &crate::move_::STRUGGLE }, None); } // TODO: Can Struggle be used if switch actions are available?

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
                    let mut minimizer_move_actions: Vec<MoveAction> = generate_move_actions(self.pokemon_by_id_mut(min_pokemon_id), self.minimizing_team);
                    let mut maximizer_move_actions: Vec<MoveAction> = generate_move_actions(self.pokemon_by_id_mut(max_pokemon_id), self.maximizing_team);

                    while !maximizer_move_actions.is_empty() {
                        let mut inner_vec = vec![];
                        self.child_matrix.push(inner_vec);
                        while !minimizer_move_actions.is_empty() {
                            let mut child_state = self.copy_game_state();
                            child_state.display_text.clear();
                            child_state.play_out_turn(vec![minimizer_move_actions.remove(0), maximizer_move_actions.remove(0)]);
                            inner_vec.push(child_state);
                        }
                    }
                }
            }
        }

        self.max_player_num_actions = self.child_matrix.len() as u8;
        self.min_player_num_actions = self.child_matrix.get(0).unwrap().len() as u8;

        for maximizer_action_index in 0..self.max_player_num_actions {
            let inner_vec = self.child_matrix.get(maximizer_action_index as usize).unwrap();
            for minimizer_action_index in 0..self.min_player_num_actions {
                self.payoff_matrix[maximizer_action_index as usize][minimizer_action_index as usize] = inner_vec.get(minimizer_action_index as usize).unwrap().heuristic_value(recursions - 1);
            }
        }

        self.calc_nash_eq();

        /// Diminishing return for future payoffs; 0.0 will ignore future payoffs entirely, while 1.0 will only account for the farthest-out payoffs.
        const GAMMA: f64 = 0.75;
        return GAMMA * self.expected_payoff + (1.0 - GAMMA) * self.heuristic_value(0);
    }

    /// Copies only the game state into a new State instance; doesn't copy metadata.
    fn copy_game_state(&self) -> State {
        State {
            pokemon_by_id: self.pokemon_by_id.clone(),
            minimizing_team: self.minimizing_team.clone(),
            maximizing_team: self.maximizing_team.clone(),
            min_pokemon_id: self.min_pokemon_id,
            max_pokemon_id: self.max_pokemon_id,
            weather: self.weather,
            terrain: self.terrain,
            turn_number: self.turn_number,
            display_text: self.display_text.clone(),
            battle_ended: self.battle_ended,
            max_player_num_actions: 0,
            min_player_num_actions: 0,
            max_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
            min_player_strategy: [0.0; MAX_ACTIONS_PER_AGENT],
            expected_payoff: 0.0,
            child_matrix: vec![],
            payoff_matrix: [[0.0; MAX_ACTIONS_PER_AGENT]; MAX_ACTIONS_PER_AGENT]
        }
    }

    pub fn pokemon_by_id(&self, id: u8) -> &Pokemon {
        &self.pokemon_by_id[id as usize]
    }

    pub fn pokemon_by_id_mut(&mut self, id: u8) -> &mut Pokemon {
        &mut self.pokemon_by_id[id as usize]
    }

    pub fn battle_end_check(&mut self) -> bool {
        if self.minimizing_team.iter().all(|id| self.pokemon_by_id(*id).current_hp == 0) {
            self.display_text.push(String::from(""));
            self.display_text.push(String::from("The maximizing team won!"));
            self.battle_ended = true;
        } else if self.maximizing_team.iter().all(|id| self.pokemon_by_id(*id).current_hp == 0) {
            self.display_text.push(String::from(""));
            self.display_text.push(String::from("The minimizing team won!"));
            self.battle_ended = true;
        }
        self.battle_ended
    }

    /// Recalculates this state's Nash equilibrium using its payoff matrix.
    /// Algorithm follows https://www.math.ucla.edu/~tom/Game_Theory/mat.pdf, section 4.5.
    fn calc_nash_eq(&mut self) {
        let m = self.max_player_num_actions as usize;
        let n = self.min_player_num_actions as usize;

        let mut tableau = [[0.0; MAX_ACTIONS_PER_AGENT + 1]; MAX_ACTIONS_PER_AGENT + 1];
        for i in 0..m {
            for j in 0..n {
                tableau[i][j] = self.payoff_matrix[i][j] + ADDED_CONSTANT;
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

        // Store solution
        for i in 0..m { self.max_player_strategy[i] = 0.0; }
        for j in 0..n { self.min_player_strategy[j] = 0.0; }
        self.expected_payoff = 1.0 / tableau[m][n] - ADDED_CONSTANT;
        for j in 0..n {
            if top_labels[j] > 0 { // If it's one of row player's labels
                self.max_player_strategy[top_labels[j] as usize - 1] = tableau[m][j] / tableau[m][n];
            }
        }
        for i in 0..m {
            if left_labels[i] < 0 { // If it's one of column player's labels
                self.min_player_strategy[(-left_labels[i]) as usize - 1] = tableau[i][n] / tableau[m][n];
            }
        }
    }
}
