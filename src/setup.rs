use rand::Rng;

use crate::{Ability, Gender, Nature};
use crate::move_::Move;
use crate::pokemon::Pokemon;
use crate::species::Species;

/// Part of a `TeamConfig`; contains all the necessary information to create a `Pokemon` object.
pub struct PokemonConfig {
    species: &'static Species,
    gender: Gender,
    nature: Nature,
    ability: Ability,
    ivs: [u8; 6],
    evs: [u8; 6],
    moves: Vec<&'static Move>,
}

impl PokemonConfig {
    pub fn new() -> PokemonConfig {
        let species = Species::random_species();
        let mut pokemon_config = PokemonConfig {
            species,
            gender: species.random_gender(),
            nature: Nature::random_nature(),
            ability: species.random_ability(),
            // TODO: Use seeded RNG
            ivs: [rand::thread_rng().gen_range(0, 32); 6],
            evs: [rand::thread_rng().gen_range(0, 253); 6],
            moves: species.random_move_set(),
        };
        pokemon_config.fix_evs();
        pokemon_config
    }

    pub fn create_pokemon(&self) -> Pokemon {
        Pokemon::new(self.species, self.gender, self.nature, self.ability, self.ivs, self.evs, &self.moves)
    }

    // TODO: Use seeded RNG
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