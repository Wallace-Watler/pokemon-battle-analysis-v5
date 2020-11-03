use crate::{Ability, Gender, Nature};
use crate::move_::Move;
use crate::pokemon::Pokemon;
use crate::species::Species;
use rand::prelude::StdRng;
use rand::Rng;

/// Part of a `TeamBuild`; contains all the necessary information to create a `Pokemon` object.
pub struct PokemonBuild {
    species: &'static Species,
    gender: Gender,
    nature: &'static str,
    ability: Ability,
    ivs: [u8; 6],
    evs: [u8; 6],
    moves: Vec<&'static Move>,
}

impl PokemonBuild {
    pub fn new(rng: &mut StdRng) -> PokemonBuild {
        let species = Species::random_species(rng);
        let mut pokemon_build = PokemonBuild {
            species,
            gender: species.random_gender(rng),
            nature: Nature::random_nature(rng).name,
            ability: species.random_ability(rng),
            ivs: [rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32)],
            evs: [rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253)],
            moves: species.random_move_set(rng),
        };
        pokemon_build.fix_evs(rng);
        pokemon_build
    }

    pub fn create_pokemon(&self) -> Pokemon {
        Pokemon::new(self.species, self.gender, Nature::id_by_name(self.nature), self.ability, self.ivs, self.evs, &self.moves)
    }

    fn fix_evs(&mut self, rng: &mut StdRng) {
        let mut ev_sum: u16 = self.evs.iter().map(|ev| *ev as u16).sum();
        while ev_sum < 510 {
            let i = rng.gen_range(0, 6);
            if self.evs[i] < 252 {
                self.evs[i] += 1;
                ev_sum += 1;
            }
        }
        while ev_sum > 510 {
            let i = rng.gen_range(0, 6);
            if self.evs[i] > 0 {
                self.evs[i] -= 1;
                ev_sum -= 1;
            }
        }
    }
}