use pokemon_battle_analysis_v5::{GameVersion, state};
use pokemon_battle_analysis_v5::state::State;
use pokemon_battle_analysis_v5::setup::PokemonConfig;
use pokemon_battle_analysis_v5::species;
use pokemon_battle_analysis_v5::move_;

#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;

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

    let bulbasaur_config = PokemonConfig::new();
    let test_pokemon = [bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon()];

    let test_state = || State {
        pokemon: test_pokemon.clone(),
        min_pokemon_id: None,
        max_pokemon_id: None,
        weather: Default::default(),
        terrain: Default::default(),
        turn_number: 0,
        display_text: vec![]
    };

    state::run_battle(test_state(), true);
}
