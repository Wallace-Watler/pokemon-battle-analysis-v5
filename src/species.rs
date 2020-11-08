use crate::{game_version, StatIndex, Ability, Gender, Type, AbilityID};
use rand::prelude::StdRng;
use rand::Rng;
use std::fs;
use std::cmp::min;
use crate::move_::{MoveID, Move};
use std::process::exit;
use json::JsonValue;

pub type SpeciesID = u16;

#[derive(Debug, Default)]
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
    pub fn id_by_name(name: &str) -> SpeciesID {
        unsafe {
            for (species_id, species) in SPECIES.iter().enumerate() {
                if species.name.eq_ignore_ascii_case(name) {
                    return species_id as SpeciesID;
                }
            }
        }
        panic!(format!("Invalid species '{}'.", name));
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

static mut SPECIES: Vec<Species> = Vec::new();

/// # Safety
/// Should be called after the game version has been set from the program input and the moves have been initialized.
pub fn initialize_species() {
    let mut path = String::from("resources/");
    path.push_str(game_version().name());
    path.push_str("/species.json");
    let species_json = fs::read_to_string(path.as_str()).unwrap_or_else(|_| panic!("Failed to read {}.", path));

    match json::parse(species_json.as_str()) {
        json::Result::Ok(parsed) => {
            match parsed {
                JsonValue::Array(array) => {
                    let extract_string = |json_value: &mut JsonValue, key: &str| -> String {
                        json_value.remove(key).as_str()
                            .unwrap_or_else(|| panic!("Invalid species.json: member\n{}\ndoes not have a valid string field '{}'", json_value.pretty(4), key))
                            .to_owned()
                    };
                    let extract_type = |json_value: &mut JsonValue, key: &str| -> Type {
                        let string = extract_string(json_value, key);
                        Type::by_name(string.as_str())
                            .unwrap_or_else(|_| panic!("Invalid species.json: '{}' in object\n{}\nis not a valid {}", string, json_value.pretty(4), key))
                    };
                    let extract_ability = |json_value: &mut JsonValue, key: &str| -> AbilityID {
                        let string = extract_string(json_value, key);
                        Ability::id_by_name(string.as_str())
                            .unwrap_or_else(|_| panic!("Invalid species.json: '{}' in object\n{}\nis not a valid {}", string, json_value.pretty(4), key))
                    };
                    let extract_u16 = |json_value: &mut JsonValue, key: &str| -> u16 {
                        json_value.remove(key).as_u16()
                            .unwrap_or_else(|| panic!("Invalid species.json: member\n{}\ndoes not have a valid u16 field '{}'", json_value.pretty(4), key))
                    };
                    let extract_bool = |json_value: &mut JsonValue, key: &str| -> bool {
                        json_value.remove(key).as_bool()
                            .unwrap_or_else(|| panic!("Invalid species.json: member\n{}\ndoes not have a valid boolean field '{}'", json_value.pretty(4), key))
                    };

                    for mut json_species in array {
                        let mut base_stats: [u8; 6] = [0, 0, 0, 0, 0, 0];
                        let json_base_stats = json_species.remove("base_stats");
                        let json_base_stats_members = json_base_stats.members();
                        if json_base_stats_members.len() != 6 { panic!("Invalid species.json: 'base_stats' in member\n{}\ndoes not contain 6 numbers", json_species.pretty(4)) }
                        for (i, json_base_stat) in json_base_stats_members.enumerate() {
                            base_stats[i] = json_base_stat.as_u8()
                                .unwrap_or_else(|| panic!("Invalid species.json: 'base_stats' in object\n{}\ncontains invalid u8 numbers", json_species.pretty(4)))
                        }

                        let mut move_pool = Vec::new();
                        let json_moves = json_species.remove("move_pool");
                        let json_moves_members = json_moves.members();
                        for json_move in json_moves_members {
                            let move_name = json_move.as_str().unwrap_or_else(|| panic!("Invalid species.json: 'move_pool' in member\n{}\ncontains invalid strings", json_species.pretty(4)));
                            match Move::id_by_name(move_name) {
                                Ok(move_) => move_pool.push(move_),
                                Err(_) => panic!("Invalid species.json: '{}' in member\n{}\nis not a valid move", move_name, json_move)
                            }
                        }

                        unsafe {
                            SPECIES.push(Species {
                                name: extract_string(&mut json_species, "name").to_owned(),
                                type1: extract_type(&mut json_species, "type1"),
                                type2: extract_type(&mut json_species, "type2"),
                                ability1: extract_ability(&mut json_species, "ability1"),
                                ability2: extract_ability(&mut json_species, "ability2"),
                                base_stats,
                                weight: extract_u16(&mut json_species, "weight"),
                                male_chance: extract_u16(&mut json_species, "male_chance"),
                                female_chance: extract_u16(&mut json_species, "female_chance"),
                                allow_duplicates: extract_bool(&mut json_species, "allow_duplicates"),
                                move_pool
                            });
                        }
                    }
                },
                _ => panic!("Invalid species JSON: not an array of objects")
            }
        },
        json::Result::Err(error) => {
            println!("{}", error);
            exit(1);
        }
    }
}
