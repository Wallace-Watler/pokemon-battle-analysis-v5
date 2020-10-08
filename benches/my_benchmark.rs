use criterion::{Criterion, BenchmarkId, criterion_group, criterion_main};
use pokemon_battle_analysis_v5::{Gender, Nature, Ability, GameVersion, species, state};
use pokemon_battle_analysis_v5::pokemon::Pokemon;
use pokemon_battle_analysis_v5::state::State;

fn ai_benchmark(c: &mut Criterion) {
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
    }

    let test_bulbasaur = Pokemon::new(unsafe { &species::BULBASAUR }, Gender::Male, Nature::Adamant, Ability::Overgrow, [31; 6], [42; 6], vec![]);
    let test_pokemon = [test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur.clone(), test_bulbasaur];

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
    c.bench_function("Pokemon AI", |b| b.iter(|| state::run_battle(test_state())));
}

criterion_group!(benches, ai_benchmark);
criterion_main!(benches);
