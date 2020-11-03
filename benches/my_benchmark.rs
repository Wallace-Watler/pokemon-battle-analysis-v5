use criterion::{Criterion, criterion_group, criterion_main};
use pokemon_battle_analysis_v5::{GameVersion, species, state, move_};
use pokemon_battle_analysis_v5::state::State;
use pokemon_battle_analysis_v5::setup::PokemonBuild;
use rand::prelude::StdRng;
use rand::SeedableRng;

fn ai_benchmark(c: &mut Criterion) {
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

    c.bench_function("Pokemon AI Setup", |b| b.iter(|| test_state()));
    c.bench_function("Pokemon AI", |b| b.iter(|| state::run_battle(test_state(), &mut rng)));
}

criterion_group!{
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = ai_benchmark
}
criterion_main!(benches);
