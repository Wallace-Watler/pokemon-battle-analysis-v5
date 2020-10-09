use rand::Rng;

use crate::{Ability, Gender, Nature};
use crate::move_::MoveV2;
use crate::pokemon::PokemonV2;
use crate::species::SpeciesV2;

/**
 * Part of a {@code TeamConfig}; contains all the necessary information to create a {@code Pokemon} object.
 */
pub struct PokemonConfigV2 {
    species: &'static SpeciesV2,
    gender: Gender,
    nature: Nature,
    ability: Ability,
    ivs: [u8; 6],
    evs: [u8; 6],
    moves: Vec<&'static MoveV2>,
}

impl PokemonConfigV2 {
    pub fn new() -> PokemonConfigV2 {
        let species = SpeciesV2::random_species();
        let mut pokemon_config = PokemonConfigV2 {
            species,
            gender: species.random_gender(),
            nature: Nature::random_nature(),
            ability: species.random_ability(),
            ivs: [rand::thread_rng().gen_range(0, 32); 6],
            evs: [rand::thread_rng().gen_range(0, 253); 6],
            moves: species.random_move_set(),
        };
        pokemon_config.fix_evs();
        pokemon_config
    }

    pub fn create_pokemon(&self) -> PokemonV2 {
        PokemonV2::new(self.species, self.gender, self.nature, self.ability, self.ivs, self.evs, &self.moves)
    }

    fn fix_evs(&mut self) {
        let mut ev_sum: u16 = self.evs.iter().map(|ev| *ev as u16).sum();
        while ev_sum < 510 {
            let i = rand::thread_rng().gen_range(0, 6);
            if self.evs[i] < 252 {
                self.evs[i] += 1;
                ev_sum += 1;
            }
        }
        while ev_sum > 510 {
            let i = rand::thread_rng().gen_range(0, 6);
            if self.evs[i] > 0 {
                self.evs[i] -= 1;
                ev_sum -= 1;
            }
        }
    }
}
