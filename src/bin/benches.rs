use pokemon_battle_analysis_v5::{GameVersion, move_, species};
use pokemon_battle_analysis_v5::battle_ai::pokemon::TeamBuild;
use pokemon_battle_analysis_v5::battle_ai::state;
use std::iter;
use std::ops::Div;
use std::time::Instant;
use rand::rngs::StdRng;
use rand::SeedableRng;
use pokemon_battle_analysis_v5::combinatorial_optim::Solver;

fn _single_battle(num_samples: u32) {
    let mut rng: StdRng = SeedableRng::from_seed([0; 32]);
    let teams: Vec<TeamBuild> = iter::repeat_with(|| TeamBuild::new(&mut rng))
        .take(2 * num_samples as usize)
        .collect();

    unsafe { state::NUM_STATE_COPIES = 0; }
    let start_time = Instant::now();
    for i in 0..num_samples as usize {
        println!("{}", i);
        state::run_battle(&teams[i], &teams[i + num_samples as usize], &mut rng);
    }

    let dur = start_time.elapsed();
    let nsc = unsafe { state::NUM_STATE_COPIES };
    println!("---- Single Battle ----");
    println!("AI level: {:?}", state::AI_LEVEL);
    println!("Num samples: {:?}", num_samples);
    println!("Elapsed time: {:?}", dur);
    println!("Num state copies: {:?}", nsc);
    println!("Avg time per battle: {:?}", dur.div(num_samples));
    println!("Avg state copies per battle: {:?}", nsc / num_samples as u64);
    println!("Avg time per state: {:?}ns\n", dur.as_nanos() / nsc as u128);
}

fn _combinatorial_optim(iters: u32) {
    let mut rng: StdRng = SeedableRng::from_seed([0; 32]);
    let mut solver = Solver::new(&mut rng);

    let start_time = Instant::now();
    for i in 0..iters {
        println!("Iters: {}", i);
        solver.do_iter(&mut rng);
    }

    let dur = start_time.elapsed();
    println!("---- Combinatorial Optimization ----");
    println!("AI level: {:?}", state::AI_LEVEL);
    println!("Iterations: {:?}", iters);
    println!("Elapsed time: {:?}", dur);
    println!("Avg time per iteration: {:?}", dur.div(iters));
}

fn main() {
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
        move_::initialize_moves();
        species::initialize_species();
    }

    //_single_battle(30);
    //_combinatorial_optim(30);
}
