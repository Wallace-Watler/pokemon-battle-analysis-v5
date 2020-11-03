#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

use pokemon_battle_analysis_v5::{GameVersion, state};
use pokemon_battle_analysis_v5::move_;
use pokemon_battle_analysis_v5::setup::PokemonBuild;
use pokemon_battle_analysis_v5::species;
use pokemon_battle_analysis_v5::state::State;
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

    let test_pokemon = [
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon(),
        bulbasaur_build.create_pokemon()
    ];

    let test_state = || State::new(test_pokemon.clone(), Default::default(), Default::default());

    state::run_battle(test_state(), &mut rng);
}
