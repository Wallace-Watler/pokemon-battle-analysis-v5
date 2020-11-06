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
    let species_json = fs::read_to_string(path.as_str()).expect(format!("Failed to read {}.", path).as_str());

    match json::parse(species_json.as_str()) {
        json::Result::Ok(parsed) => {
            match parsed {
                JsonValue::Array(array) => {
                    for member in array {
                        let member_pretty = member.pretty(4);
                        match member {
                            JsonValue::Object(object) => {
                                let extract_string = |key: &str| -> &str {
                                    object.get(key)
                                        .expect(format!("Invalid species JSON: object\n{}\ndoes not have a '{}' field", member_pretty, key).as_str())
                                        .as_str()
                                        .expect(format!("Invalid species JSON: '{}' in object\n{}\nis not a String", key, member_pretty).as_str())
                                };
                                let extract_type = |key: &str| -> Type {
                                    let string = extract_string(key);
                                    Type::by_name(string)
                                        .expect(format!("Invalid species JSON: '{}' in object\n{}\nis not a valid {}", string, member_pretty, key).as_str())
                                };
                                let extract_ability = |key: &str| -> AbilityID {
                                    let string = extract_string(key);
                                    Ability::id_by_name(string)
                                        .expect(format!("Invalid species JSON: '{}' in object\n{}\nis not a valid {}", string, member_pretty, key).as_str())
                                };
                                let extract_u16 = |key: &str| -> u16 {
                                    object.get(key)
                                        .expect(format!("Invalid species JSON: object\n{}\ndoes not have a '{}' field", member_pretty, key).as_str())
                                        .as_u16()
                                        .expect(format!("Invalid species JSON: '{}' in object\n{}\nis not a valid u16 number", key, member_pretty).as_str())
                                };

                                let mut base_stats: [u8; 6] = [0, 0, 0, 0, 0, 0];
                                match object.get("base_stats") {
                                    Some(value) => {
                                        match value {
                                            JsonValue::Array(array) => {
                                                if array.len() != 6 { panic!(format!("Invalid species JSON: 'base_stats' in object\n{}\ndoes not contain 6 numbers", member_pretty)) }
                                                for (i, member) in array.iter().enumerate() {
                                                    base_stats[i] = member.as_u8()
                                                        .expect(format!("Invalid species JSON: 'base_stats' in object\n{}\ncontains invalid u8 numbers", member_pretty).as_str())
                                                }
                                            },
                                            _ => panic!(format!("Invalid species JSON: 'base_stats' in object\n{}\nis not an array", member_pretty))
                                        }
                                    },
                                    None => panic!(format!("Invalid species JSON: object\n{}\ndoes not have a 'base_stats' field", member_pretty))
                                }

                                let mut move_pool = Vec::new();
                                match object.get("move_pool") {
                                    Some(value) => {
                                        match value {
                                            JsonValue::Array(array) => {
                                                for member in array {
                                                    match member.as_str() {
                                                        Some(string) => {
                                                            match Move::id_by_name(string) {
                                                                Ok(move_) => move_pool.push(move_),
                                                                Err(_) => panic!(format!("Invalid species JSON: '{}' in object\n{}\nis not a valid move", string, member_pretty))
                                                            }
                                                        },
                                                        None => panic!(format!("Invalid species JSON: 'move_pool' in object\n{}\ncontains invalid strings", member_pretty))
                                                    }
                                                }
                                            },
                                            _ => panic!(format!("Invalid species JSON: 'move_pool' in object\n{}\nis not an array", member_pretty))
                                        }
                                    },
                                    None => panic!(format!("Invalid species JSON: object\n{}\ndoes not have a 'move_pool' field", member_pretty))
                                }

                                unsafe {
                                    SPECIES.push(Species {
                                        name: extract_string("name").to_owned(),
                                        type1: extract_type("type1"),
                                        type2: extract_type("type2"),
                                        ability1: extract_ability("ability1"),
                                        ability2: extract_ability("ability2"),
                                        base_stats,
                                        weight: extract_u16("weight"),
                                        male_chance: extract_u16("male_chance"),
                                        female_chance: extract_u16("female_chance"),
                                        allow_duplicates: object.get("allow_duplicates")
                                            .expect(format!("Invalid species JSON: object\n{}\ndoes not have an 'allow_duplicates' field", member_pretty).as_str())
                                            .as_bool()
                                            .expect(format!("Invalid species JSON: 'allow_duplicates' in object\n{}\nis not a boolean", member_pretty).as_str()),
                                        move_pool
                                    });
                                }
                            },
                            _ => panic!(format!("Invalid species JSON: member\n{}\nis not an object", member_pretty))
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

    /*
    let bulbasaur = Species {
        name: "Bulbasaur",
        first_type: Type::Grass,
        second_type: Type::Poison,
        first_ability: "Overgrow",
        second_ability: if game_version().gen() <= 5 { "None" } else { "Chlorophyll" },
        base_stats: [45, 49, 49, 65, 65, 45],
        weight: 69,
        male_chance: 875,
        female_chance: 125,
        allow_duplicates: true,
        move_pool: match game_version() {
            GameVersion::FRLG => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, RAZOR_LEAF, SWEET_SCENT, GROWTH, SYNTHESIS, SOLAR_BEAM, TOXIC, BULLET_SEED, HIDDEN_POWER, SUNNY_DAY, PROTECT, GIGA_DRAIN, FRUSTRATION, RETURN, DOUBLE_TEAM, SLUDGE_BOMB, FACADE, SECRET_POWER, REST, ATTRACT, CUT, STRENGTH, FLASH, ROCK_SMASH, CHARM, CURSE, GRASS_WHISTLE, LIGHT_SCREEN, MAGICAL_LEAF, PETAL_DANCE, SAFEGUARD, SKULL_BASH, BODY_SLAM, DOUBLE-EDGE, MIMIC, SUBSTITUTE, SWORDS_DANCE*/],
            GameVersion::HGSS => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, SWEET_SCENT, GROWTH, DOUBLE-EDGE, WORRY_SEED, SYNTHESIS, SEED_BOMB, TOXIC, BULLET_SEED, HIDDEN_POWER, SUNNY_DAY, PROTECT, GIGA_DRAIN, SOLAR_BEAM, RETURN, DOUBLE_TEAM, SLUDGE_BOMB, FACADE, SECRET_POWER, REST, ATTRACT, ENERGY_BALL, ENDURE, FLASH, SWORDS_DANCE, CAPTIVATE, SLEEP_TALK, NATURAL_GIFT, GRASS_KNOT, SWAGGER, SUBSTITUTE, CUT, STRENGTH, ROCK_SMASH, AMNESIA, CHARM, CURSE, GRASS_WHISTLE, INGRAIN, LEAF_STORM, MAGICAL_LEAF, NATURE_POWER, PETAL_DANCE, POWER_WHIP, SAFEGUARD, SKULL_BASH, SLUDGE, FURY_CUTTER, HEADBUTT, KNOCK_OFF, MUD-SLAP, SNORE, STRING_SHOT*/],
            GameVersion::XY => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, SWEET_SCENT, GROWTH, DOUBLE-EDGE, WORRY_SEED, SYNTHESIS, SEED_BOMB, TOXIC, VENOSHOCK, HIDDEN_POWER, SUNNY_DAY, LIGHT_SCREEN, PROTECT, SAFEGUARD, SOLAR_BEAM, RETURN, DOUBLE_TEAM, SLUDGE_BOMB, FACADE, REST, ATTRACT, ROUND, ECHOED_VOICE, ENERGY_BALL, FLASH, SWORDS_DANCE, GRASS_KNOT, SWAGGER, SLEEP_TALK, SUBSTITUTE, ROCK_SMASH, NATURE_POWER, CONFIDE, CUT, STRENGTH, AMNESIA, CHARM, CURSE, ENDURE, GIGA_DRAIN, GRASS_WHISTLE, GRASSY_TERRAIN, INGRAIN, LEAF_STORM, MAGICAL_LEAF, PETAL_DANCE, POWER_WHIP, SKULL_BASH, SLUDGE*/],
            GameVersion::LGPLGE => vec![&move_::TACKLE, &move_::GROWL, &move_::VINE_WHIP, &move_::LEECH_SEED/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, GROWTH, DOUBLE-EDGE, HEADBUTT, REST, LIGHT_SCREEN, PROTECT, SUBSTITUTE, REFLECT, FACADE, TOXIC, OUTRAGE, SOLAR_BEAM, SLUDGE_BOMB, MEGA_DRAIN*/],
            //GameVersion::PIXELMON(_, _) => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, SWEET_SCENT, GROWTH, DOUBLE-EDGE, WORRY_SEED, SYNTHESIS, SEED_BOMB, TOXIC, VENOSHOCK, HIDDEN_POWER, SUNNY_DAY, LIGHT_SCREEN, PROTECT, SAFEGUARD, SOLAR_BEAM, RETURN, DOUBLE_TEAM, REFLECT, SLUDGE_BOMB, FACADE, REST, ATTRACT, ROUND, ECHOED_VOICE, ENERGY_BALL, FALSE_SWIPE, FLASH, SWORDS_DANCE, WORK_UP, GRASS_KNOT, SWAGGER, SUBSTITUTE, ROCK_SMASH, RAZOR_WIND, BODY_SLAM, RAGE, MEGA_DRAIN, MIMIC, BIDE, SKULL_BASH, HEADBUTT, CURSE, SNORE, GIGA_DRAIN, ENDURE, MUD-SLAP, SLEEP_TALK, DEFENSE_CURL, FURY_CUTTER, BULLET_SEED, SECRET_POWER, CAPTIVATE, NATURAL_GIFT, NATURE_POWER, CONFIDE, CUT, STRENGTH, ANCIENT_POWER, BIND, BLOCK, FRENZY_PLANT, GRASS_PLEDGE, KNOCK_OFF, STRING_SHOT, AMNESIA, CHARM, GRASS_WHISTLE, GRASSY_TERRAIN, INGRAIN, LEAF_STORM, MAGICAL_LEAF, PETAL_DANCE, POWER_WHIP, SLUDGE*/],
            _ => Default::default()
        }
    };
    SPECIES.push(bulbasaur);*/
}
