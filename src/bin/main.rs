#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

use pokemon_battle_analysis_v5::{GameVersion, state};
use pokemon_battle_analysis_v5::move_;
use pokemon_battle_analysis_v5::setup::PokemonConfig;
use pokemon_battle_analysis_v5::species;
use pokemon_battle_analysis_v5::state::State;
use rand::SeedableRng;
use rand::prelude::StdRng;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

//use tcmalloc::TCMalloc;

//#[global_allocator]
//static GLOBAL: TCMalloc = TCMalloc;

fn main() {
    //let args: Vec<String> = env::args().collect();

    // TODO: Parse game version from args
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
        move_::initialize_moves();
        species::initialize_species();
    }

    let mut rng: StdRng = SeedableRng::from_seed([0; 32]);

    let bulbasaur_config = PokemonConfig::new(&mut rng);

    let test_pokemon = [
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon()),
        Box::new(bulbasaur_config.create_pokemon())
    ];

    let test_state = || State {
        pokemon: test_pokemon.clone(),
        min_pokemon_id: None,
        max_pokemon_id: None,
        weather: Default::default(),
        terrain: Default::default(),
        turn_number: 0,
        display_text: Vec::new(),
        children: Vec::new(),
        num_maximizer_actions: 0,
        num_minimizer_actions: 0
    };

    state::run_battle_v2(test_state(), &mut rng);

    println!();
}
