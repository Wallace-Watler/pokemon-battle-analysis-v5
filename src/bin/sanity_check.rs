use pokemon_battle_analysis_v5::{GameVersion, move_, species};
use pokemon_battle_analysis_v5::battle_ai::pokemon::TeamBuild;
use pokemon_battle_analysis_v5::battle_ai::state;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn main() {
    unsafe {
        pokemon_battle_analysis_v5::GAME_VERSION = GameVersion::XY;
        move_::initialize_moves();
        species::initialize_species();
    }

    let mut rng: StdRng = SeedableRng::from_seed([0; 32]);
    state::run_battle(&TeamBuild::new(&mut rng), &TeamBuild::new(&mut rng), &mut rng);
}
