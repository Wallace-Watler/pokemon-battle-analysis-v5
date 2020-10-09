use criterion::{Criterion, criterion_group, criterion_main};
use pokemon_battle_analysis_v5::{GameVersion, species, state, move_};
use pokemon_battle_analysis_v5::state::{State, StateV2};
use pokemon_battle_analysis_v5::setup::{PokemonConfig, PokemonConfigV2};

fn ai_benchmark(c: &mut Criterion) {
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

    c.bench_function("Pokemon AI Setup", |b| b.iter(|| test_state()));
    c.bench_function("Pokemon AI", |b| b.iter(|| state::run_battle(test_state(), false)));
}

fn ai_benchmark_v2(c: &mut Criterion) {
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

    c.bench_function("Pokemon AI Setup", |b| b.iter(|| test_state()));
    c.bench_function("Pokemon AI", |b| b.iter(|| state::run_battle_v2(test_state(), false)));
}

criterion_group!(benches, ai_benchmark, ai_benchmark_v2);
criterion_main!(benches);
