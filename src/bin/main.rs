use pokemon_battle_analysis_v5::{GameVersion, state};
use pokemon_battle_analysis_v5::state::State;
use pokemon_battle_analysis_v5::setup::PokemonConfig;
use pokemon_battle_analysis_v5::species;
use pokemon_battle_analysis_v5::move_;

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

    let test_state = State {
        pokemon: test_pokemon,
        min_pokemon_id: None,
        max_pokemon_id: None,
        weather: Default::default(),
        terrain: Default::default(),
        turn_number: 0,
        display_text: vec![]
    };

    let battle_value = state::run_battle(test_state, true);
    println!("Battle value: {}", battle_value);
}
