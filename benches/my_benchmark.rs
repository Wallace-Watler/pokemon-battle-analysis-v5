use pokemon_battle_analysis_v5::GameVersion;
use pokemon_battle_analysis_v5::move_;
use pokemon_battle_analysis_v5::species;
use pokemon_battle_analysis_v5::battle_ai::state;
use pokemon_battle_analysis_v5::battle_ai::pokemon::PokemonBuild;
use criterion::{Criterion, criterion_group, criterion_main};
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
        bulbasaur_build.clone()
    ];

    c.bench_function("Pokemon AI", |b| b.iter(|| state::run_battle(bulbasaur_builds.clone(), &mut rng)));
}

criterion_group!{
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = ai_benchmark
}
criterion_main!(benches);
