use crate::battle_ai::state::{State, Agent};
use crate::battle_ai::state;

impl State {
    /// Copies only the game state into a new State instance; doesn't copy the child matrix or display text.
    fn copy_game_state(&self) -> State {
        unsafe { state::NUM_STATE_COPIES += 1; }

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
            match max_action {
                Action::Move { .. } => child.max.consecutive_switches = 0,
                Action::Switch { .. } => child.max.consecutive_switches += 1,
                Action::Nop => {}
            }
            match min_action {
                Action::Move { .. } => child.min.consecutive_switches = 0,
                Action::Switch { .. } => child.min.consecutive_switches += 1,
                Action::Nop => {}
            }
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
    match state.max.on_field.zip(state.min.on_field) {
        None => agents_choose_pokemon_to_send_out(state),
        Some((max_pokemon_id, min_pokemon_id)) => { // Agents must choose actions for each Pokemon
            let max_actions = gen_actions_for_user(state, rng, max_pokemon_id);
            let min_actions = gen_actions_for_user(state, rng, min_pokemon_id);
            state.max.actions = max_actions;
            state.min.actions = min_actions;

            state.max.actions.sort_unstable_by(|act1, act2| action_cmp(act1, act2));
            state.min.actions.sort_unstable_by(|act1, act2| action_cmp(act1, act2));
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
    let mut actions: Vec<Action> = Vec::new();

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
