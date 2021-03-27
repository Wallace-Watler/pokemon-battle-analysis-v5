use pokemon_battle_analysis_v5::{GameVersion, move_, species};
use pokemon_battle_analysis_v5::battle_ai::pokemon::TeamBuild;
use pokemon_battle_analysis_v5::battle_ai::state;
use std::iter;
use std::ops::Div;
use std::time::Instant;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn single_battle(num_samples: u32) {
    let mut rng: StdRng = SeedableRng::from_seed([0; 32]);
    let teams: Vec<TeamBuild> = iter::repeat_with(|| TeamBuild::new(&mut rng))
        .take(2 * num_samples as usize)
        .collect();

    unsafe { state::NUM_STATE_COPIES = 0; }
    let start_time = Instant::now();
    for i in 0..num_samples as usize {
        println!("{}", i);
        state::run_battle(&teams[i], &teams[i + 1], &mut rng);
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

fn main() {
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
        move_::initialize_moves();
        species::initialize_species();
    }

    single_battle(30);
}
