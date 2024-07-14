use std::cmp::max;
use std::fmt::Debug;
use std::intrinsics::transmute;
use std::ops::AddAssign;

use num::{One, Zero};
use rand::prelude::StdRng;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::move_::MoveCategory;

pub mod battle_ai;
pub mod combinatorial_optim;

fn choose_weighted_index(weights: &[f64], rng: &mut StdRng) -> usize {
    if weights.is_empty() || weights.iter().any(|d| !almost::zero(*d) && *d < 0.0) {
        panic!("Weights must be non-negative. Given weights: {:?}", weights);
    }

    let mut d = rng.gen_range::<f64, f64, f64>(0.0, weights.iter().sum());
    for (i, &weight) in weights.iter().enumerate() {
        if d < weight { return i; }
        d -= weight;
    }
    weights.len() - 1
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
