#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

use pokemon_battle_analysis_v5::GameVersion;
use pokemon_battle_analysis_v5::move_;
use pokemon_battle_analysis_v5::species;
use pokemon_battle_analysis_v5::battle_ai::state;
use pokemon_battle_analysis_v5::battle_ai::pokemon::PokemonBuild;
use rand::SeedableRng;
use rand::prelude::StdRng;

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

    let bulbasaur_build = PokemonBuild::new(&mut rng);
    let bulbasaur_builds = [
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build.clone(),
        bulbasaur_build
    ];

    state::run_battle(bulbasaur_builds, &mut rng);
}
