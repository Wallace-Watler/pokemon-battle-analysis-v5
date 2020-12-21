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

    let mut solver = match fs::read_to_string("solver.json") {
        Ok(solver_json) => serde_json::from_str(solver_json.as_str()).unwrap(),
        Err(_) => {
            println!("Warning: Could not read solver.json. Creating a new solver.");
            Solver::new()
        }
    };

    let mut  i = 0;
    loop {
        println!("{}", i);
        i += 1;
        solver.do_iter(0.99, 30, &mut rng);
        fs::write("solver.json", serde_json::to_string_pretty(&solver).unwrap()).unwrap();

        let pb_header = "species,gender,nature,ability,iv_1,iv_2,iv_3,iv_4,iv_5,iv_6,ev_1,ev_2,ev_3,ev_4,ev_5,ev_6,move_1,move_2,move_3,move_4";
        let mut tb_header = vec!["fitness", "fit_variance", "num_samples"];
        for _ in 0..6 {
            for s in pb_header.split(',') { tb_header.push(s); }
        }

        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_path("best_solutions.csv").unwrap();
        writer.write_record(&tb_header).unwrap();
        for sol in solver.best_solutions() {
            writer.serialize(sol).unwrap();
        }
    }
}
