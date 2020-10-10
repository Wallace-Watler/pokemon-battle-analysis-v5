use rand::Rng;

use crate::{Ability, game_version, GameVersion, Gender, StatIndex, Type};
use crate::move_::Move;
use crate::move_;

#[derive(Debug, Default)]
pub struct Species {
    /// True if multiple Pokemon of this species can be obtained in-game.
    allow_duplicates: bool,
    pub name: &'static str,
    pub first_type: Type,
    pub second_type: Type,
    first_ability: Ability,
    second_ability: Ability,
    base_stats: [u8; 6],
    /// In tenths of a kg.
    weight: u16,
    /// Per thousand.
    male_chance: u16,
    /// Per thousand.
    female_chance: u16,
    move_pool: Vec<&'static Move>,
}

impl Species {
    pub fn random_species() -> &'static Species {
        unsafe {
            // TODO: Use seedable RNG
            SPECIES.get_unchecked(rand::thread_rng().gen_range(0, SPECIES.len()))
        }
    }

    pub fn base_stat(&self, stat_index: StatIndex) -> u8 {
        self.base_stats[stat_index.as_usize()]
    }

    // TODO: Use seedable RNG
    pub fn random_gender(&self) -> Gender {
        let i = rand::thread_rng().gen_range(0, 1000);
        if i < self.female_chance {
            Gender::Female
        } else if i < self.female_chance + self.male_chance {
            Gender::Male
        } else {
            Gender::None
        }
    }

    // TODO: Use seedable RNG
    pub fn random_ability(&self) -> Ability {
        if self.second_ability != Ability::None && rand::thread_rng().gen_bool(0.5) {
            self.second_ability
        } else {
            self.first_ability
        }
    }

    // TODO: Use seedable RNG
    pub fn random_move_set(&self) -> Vec<&'static Move> {
        if self.move_pool.len() <= 4 { return self.move_pool.clone(); }

        let mut moves: Vec<&'static Move> = vec![];
        while moves.len() < 4 {
            let random_choice = self.move_pool.get(rand::thread_rng().gen_range(0, self.move_pool.len())).unwrap();
            if !moves.contains(random_choice) {
                moves.push(*random_choice);
            }
        }
        moves
    }
}

static mut SPECIES: Vec<&Species> = vec![];

pub static mut BULBASAUR: Species = Species {
    allow_duplicates: true,
    name: "Bulbasaur",
    first_type: Type::Grass,
    second_type: Type::Poison,
    first_ability: Ability::Overgrow,
    second_ability: Ability::None,
    base_stats: [45, 49, 49, 65, 65, 45],
    weight: 69,
    male_chance: 875,
    female_chance: 125,
    move_pool: vec![],
};

/// # Safety
/// Should only be called after the game version has been set from the program input and the moves have been initialized.
pub unsafe fn initialize_species() {
    BULBASAUR = Species {
        allow_duplicates: true,
        name: "Bulbasaur",
        first_type: Type::Grass,
        second_type: Type::Poison,
        first_ability: Ability::Overgrow,
        second_ability: if game_version().gen() <= 5 { Ability::None } else { Ability::Chlorophyll },
        base_stats: [45, 49, 49, 65, 65, 45],
        weight: 69,
        male_chance: 875,
        female_chance: 125,
        move_pool: match game_version() {
            GameVersion::FRLG => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, RAZOR_LEAF, SWEET_SCENT, GROWTH, SYNTHESIS, SOLAR_BEAM, TOXIC, BULLET_SEED, HIDDEN_POWER, SUNNY_DAY, PROTECT, GIGA_DRAIN, FRUSTRATION, RETURN, DOUBLE_TEAM, SLUDGE_BOMB, FACADE, SECRET_POWER, REST, ATTRACT, CUT, STRENGTH, FLASH, ROCK_SMASH, CHARM, CURSE, GRASS_WHISTLE, LIGHT_SCREEN, MAGICAL_LEAF, PETAL_DANCE, SAFEGUARD, SKULL_BASH, BODY_SLAM, DOUBLE-EDGE, MIMIC, SUBSTITUTE, SWORDS_DANCE*/],
            GameVersion::HGSS => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, SWEET_SCENT, GROWTH, DOUBLE-EDGE, WORRY_SEED, SYNTHESIS, SEED_BOMB, TOXIC, BULLET_SEED, HIDDEN_POWER, SUNNY_DAY, PROTECT, GIGA_DRAIN, SOLAR_BEAM, RETURN, DOUBLE_TEAM, SLUDGE_BOMB, FACADE, SECRET_POWER, REST, ATTRACT, ENERGY_BALL, ENDURE, FLASH, SWORDS_DANCE, CAPTIVATE, SLEEP_TALK, NATURAL_GIFT, GRASS_KNOT, SWAGGER, SUBSTITUTE, CUT, STRENGTH, ROCK_SMASH, AMNESIA, CHARM, CURSE, GRASS_WHISTLE, INGRAIN, LEAF_STORM, MAGICAL_LEAF, NATURE_POWER, PETAL_DANCE, POWER_WHIP, SAFEGUARD, SKULL_BASH, SLUDGE, FURY_CUTTER, HEADBUTT, KNOCK_OFF, MUD-SLAP, SNORE, STRING_SHOT*/],
            GameVersion::XY => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, SWEET_SCENT, GROWTH, DOUBLE-EDGE, WORRY_SEED, SYNTHESIS, SEED_BOMB, TOXIC, VENOSHOCK, HIDDEN_POWER, SUNNY_DAY, LIGHT_SCREEN, PROTECT, SAFEGUARD, SOLAR_BEAM, RETURN, DOUBLE_TEAM, SLUDGE_BOMB, FACADE, REST, ATTRACT, ROUND, ECHOED_VOICE, ENERGY_BALL, FLASH, SWORDS_DANCE, GRASS_KNOT, SWAGGER, SLEEP_TALK, SUBSTITUTE, ROCK_SMASH, NATURE_POWER, CONFIDE, CUT, STRENGTH, AMNESIA, CHARM, CURSE, ENDURE, GIGA_DRAIN, GRASS_WHISTLE, GRASSY_TERRAIN, INGRAIN, LEAF_STORM, MAGICAL_LEAF, PETAL_DANCE, POWER_WHIP, SKULL_BASH, SLUDGE*/],
            GameVersion::LGPLGE => vec![&move_::TACKLE, &move_::GROWL, &move_::VINE_WHIP, &move_::LEECH_SEED/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, GROWTH, DOUBLE-EDGE, HEADBUTT, REST, LIGHT_SCREEN, PROTECT, SUBSTITUTE, REFLECT, FACADE, TOXIC, OUTRAGE, SOLAR_BEAM, SLUDGE_BOMB, MEGA_DRAIN*/],
            GameVersion::PIXELMON(_, _) => vec![&move_::TACKLE, &move_::GROWL, &move_::LEECH_SEED, &move_::VINE_WHIP/*, POISON_POWDER, SLEEP_POWDER, TAKE_DOWN, RAZOR_LEAF, SWEET_SCENT, GROWTH, DOUBLE-EDGE, WORRY_SEED, SYNTHESIS, SEED_BOMB, TOXIC, VENOSHOCK, HIDDEN_POWER, SUNNY_DAY, LIGHT_SCREEN, PROTECT, SAFEGUARD, SOLAR_BEAM, RETURN, DOUBLE_TEAM, REFLECT, SLUDGE_BOMB, FACADE, REST, ATTRACT, ROUND, ECHOED_VOICE, ENERGY_BALL, FALSE_SWIPE, FLASH, SWORDS_DANCE, WORK_UP, GRASS_KNOT, SWAGGER, SUBSTITUTE, ROCK_SMASH, RAZOR_WIND, BODY_SLAM, RAGE, MEGA_DRAIN, MIMIC, BIDE, SKULL_BASH, HEADBUTT, CURSE, SNORE, GIGA_DRAIN, ENDURE, MUD-SLAP, SLEEP_TALK, DEFENSE_CURL, FURY_CUTTER, BULLET_SEED, SECRET_POWER, CAPTIVATE, NATURAL_GIFT, NATURE_POWER, CONFIDE, CUT, STRENGTH, ANCIENT_POWER, BIND, BLOCK, FRENZY_PLANT, GRASS_PLEDGE, KNOCK_OFF, STRING_SHOT, AMNESIA, CHARM, GRASS_WHISTLE, GRASSY_TERRAIN, INGRAIN, LEAF_STORM, MAGICAL_LEAF, PETAL_DANCE, POWER_WHIP, SLUDGE*/],
            _ => Default::default()
        },
    };
    SPECIES.push(&BULBASAUR);
}

fn _by_name(name: &str) -> &Species {
    unsafe {
        if name.eq_ignore_ascii_case(BULBASAUR.name) { return &BULBASAUR; }
        panic!(format!("Name '{}' not recognized.", name));
    }
}
