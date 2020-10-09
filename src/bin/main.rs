use pokemon_battle_analysis_v5::{GameVersion, state};
use pokemon_battle_analysis_v5::state::{State, StateV2};
use pokemon_battle_analysis_v5::setup::{PokemonConfig, PokemonConfigV2};
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
        move_::initialize_moves_v2();
        species::initialize_species_v2();
    }

    let bulbasaur_config = PokemonConfigV2::new();
    let test_pokemon = [bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon(), bulbasaur_config.create_pokemon()];

    let test_state = || StateV2 {
        pokemon: test_pokemon.clone(),
        min_pokemon_id: None,
        max_pokemon_id: None,
        weather: Default::default(),
        terrain: Default::default(),
        turn_number: 0,
        display_text: vec![],
        children: vec![],
        num_maximizer_actions: 0,
        num_minimizer_actions: 0
    };

    state::run_battle_v2(test_state(), true);
}
