#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

use pokemon_battle_analysis_v5::combinatorial_optim::Solver;
use pokemon_battle_analysis_v5::GameVersion;
use pokemon_battle_analysis_v5::move_;
use pokemon_battle_analysis_v5::species;
use csv::WriterBuilder;
use rand::SeedableRng;
use rand::prelude::StdRng;
use std::fs;
use std::iter;
use std::time::Instant;
use pokemon_battle_analysis_v5::battle_ai::pokemon::TeamBuild;
use pokemon_battle_analysis_v5::battle_ai::state;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn main() {
    //let args: Vec<String> = env::args().collect();

    // TODO: Parse game version from args
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
        move_::initialize_moves();
        species::initialize_species();
    }

    let mut rng: StdRng = SeedableRng::from_seed([0; 32]);

    let num_battles = 100;
    let meta: Vec<TeamBuild> = iter::repeat_with(|| TeamBuild::new(&mut rng)).take(2 * num_battles).collect();
    let start_time = Instant::now();
    for i in 0..num_battles {
        println!("{}", i);
        state::run_battle(&meta[i], &meta[i + num_battles], &mut rng);
    }
    println!("Elapsed time: {:?}", start_time.elapsed());
    println!("Num state copies: {:?}", unsafe { state::NUM_STATE_COPIES });

    /*let mut solver = match fs::read_to_string("solver_state.json") {
        Ok(solver_json) => serde_json::from_str(solver_json.as_str()).unwrap(),
        Err(_) => {
            println!("Warning: Could not read solver_state.json. Creating a new solver.");
            Solver::new(&mut rng)
        }
    };

    let start_time = Instant::now();
    let mut  i = 0;
    while i < 1 {
        println!("{}", i);
        i += 1;

        solver.do_iter(&mut rng);
        fs::write("solver_state.json", serde_json::to_string_pretty(&solver).unwrap()).unwrap();

        let pb_header = "species,gender,nature,ability,iv_1,iv_2,iv_3,iv_4,iv_5,iv_6,ev_1,ev_2,ev_3,ev_4,ev_5,ev_6,move_1,move_2,move_3,move_4";
        let mut tb_header = vec!["fitness", "fit_variance", "num_samples", "prob_worse_than_best"];
        for _ in 0..6 {
            for s in pb_header.split(',') { tb_header.push(s); }
        }

        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_path("maximizer_meta.csv").unwrap();
        writer.write_record(&tb_header).unwrap();
        for sol in solver.maximizer_meta() {
            writer.serialize(sol).unwrap();
        }
    }
    println!("Elapsed time: {:?}", start_time.elapsed());*/
}
