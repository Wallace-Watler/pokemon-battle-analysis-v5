use pokemon_battle_analysis_v5::{Gender, Nature, Ability, GameVersion, species, state};
use pokemon_battle_analysis_v5::pokemon::Pokemon;
use pokemon_battle_analysis_v5::state::State;

fn main() {
    //let args: Vec<String> = env::args().collect();

    // TODO: Parse game version from args
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
    }

    let test_bulbasaur = Pokemon::new(unsafe { &species::BULBASAUR }, Gender::Male, Nature::Adamant, Ability::Overgrow, [31; 6], [42; 6], vec![]);
    let test_pokemon = [test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur];

    let test_state = State {
        pokemon: test_pokemon,
        min_pokemon_id: None,
        max_pokemon_id: None,
        weather: Default::default(),
        terrain: Default::default(),
        turn_number: 0,
        display_text: vec![]
    };

    let battle_value = state::run_battle(test_state);
    println!("Battle value: {}", battle_value);
}
