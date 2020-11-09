use crate::{Gender, Ability, AbilityID, Type, StatIndex, game_version};
use crate::move_::{MoveID, Move};
use rand::prelude::StdRng;
use rand::Rng;
use serde::Deserialize;
use serde::export::TryFrom;
use std::fs;
use std::cmp::min;

pub type SpeciesID = u8;

#[derive(Debug, Default, Deserialize)]
#[serde(try_from = "SpeciesSerde")]
pub struct Species {
    /// True if multiple Pokemon of this species can be obtained in-game.
    name: String,
    type1: Type,
    type2: Type,
    ability1: AbilityID,
    ability2: AbilityID,
    base_stats: [u8; 6],
    /// In tenths of a kg.
    weight: u16,
    /// Per thousand.
    male_chance: u16,
    /// Per thousand.
    female_chance: u16,
    allow_duplicates: bool,
    move_pool: Vec<MoveID>
}

impl Species {
    pub fn id_by_name(name: &str) -> Result<SpeciesID, String> {
        unsafe {
            for (species_id, species) in SPECIES.iter().enumerate() {
                if species.name.eq_ignore_ascii_case(name) {
                    return Ok(species_id as SpeciesID);
                }
            }
        }
        Err(format!("invalid species '{}'.", name))
    }

    fn by_id(species_id: SpeciesID) -> &'static Species {
        unsafe {
            &SPECIES[species_id as usize]
        }
    }

    pub fn name(species: SpeciesID) -> &'static str {
        Species::by_id(species).name.as_str()
    }

    pub fn type1(species: SpeciesID) -> Type {
        Species::by_id(species).type1
    }

    pub fn type2(species: SpeciesID) -> Type {
        Species::by_id(species).type2
    }

    pub fn base_stat(species: SpeciesID, stat_index: StatIndex) -> u8 {
        Species::by_id(species).base_stats[stat_index.as_usize()]
    }

    pub fn random_species(rng: &mut StdRng) -> SpeciesID {
        unsafe {
            rng.gen_range(0, SPECIES.len() as SpeciesID)
        }
    }

    pub fn random_gender(species: SpeciesID, rng: &mut StdRng) -> Gender {
        let female_chance = Species::by_id(species).female_chance;
        let male_chance = Species::by_id(species).male_chance;
        let i = rng.gen_range(0, 1000);
        if i < female_chance {
            Gender::Female
        } else if i < female_chance + male_chance {
            Gender::Male
        } else {
            Gender::None
        }
    }

    pub fn random_ability(species: SpeciesID, rng: &mut StdRng) -> AbilityID {
        let ability1 = Species::by_id(species).ability1;
        let ability2 = Species::by_id(species).ability2;
        if ability2 != Ability::id_by_name("None").unwrap() && rng.gen_bool(0.5) {
            ability2
        } else {
            ability1
        }
    }

    pub fn random_move_set(species: SpeciesID, rng: &mut StdRng) -> Vec<MoveID> {
        let move_pool = &Species::by_id(species).move_pool;
        let mut move_set: Vec<MoveID> = vec![];
        while move_set.len() < min(4, move_pool.len()) {
            let random_choice = move_pool[rng.gen_range(0, move_pool.len())];
            if !move_set.contains(&random_choice) {
                move_set.push(random_choice);
            }
        }
        move_set
    }
}

impl TryFrom<SpeciesSerde<'_>> for Species {
    type Error = String;

    fn try_from(species_serde: SpeciesSerde) -> Result<Self, Self::Error> {
        let mut move_pool = Vec::new();
        for move_name in species_serde.move_pool {
            move_pool.push(Move::id_by_name(move_name)?);
        }

        Ok(
            Species {
                name: species_serde.name.to_owned(),
                type1: species_serde.type1,
                type2: species_serde.type2,
                ability1: Ability::id_by_name(species_serde.ability1)?,
                ability2: Ability::id_by_name(species_serde.ability2)?,
                base_stats: species_serde.base_stats,
                weight: species_serde.weight,
                male_chance: species_serde.male_chance,
                female_chance: species_serde.female_chance,
                allow_duplicates: species_serde.allow_duplicates,
                move_pool
            }
        )
    }
}

static mut SPECIES: Vec<Species> = Vec::new();

#[derive(Deserialize)]
struct SpeciesSerde<'d> {
    name: &'d str,
    type1: Type,
    type2: Type,
    ability1: &'d str,
    ability2: &'d str,
    base_stats: [u8; 6],
    weight: u16,
    male_chance: u16,
    female_chance: u16,
    allow_duplicates: bool,
    move_pool: Vec<&'d str>
}

/// # Safety
/// Should be called after the game version has been set from the program input and the moves have been initialized.
pub fn initialize_species() {
    let mut path = String::from("resources/");
    path.push_str(game_version().name());
    path.push_str("/species.json");
    let species_json = fs::read_to_string(path.as_str())
        .unwrap_or_else(|_| panic!("Failed to read {}.", path));
    unsafe {
        SPECIES = serde_json::from_str(species_json.as_str())
            .unwrap_or_else(|err| panic!("Error parsing species.json: {}", err));
    }
}
