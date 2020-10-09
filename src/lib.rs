extern crate strum;
extern crate strum_macros;

use std::cmp::max;
use std::fmt::Debug;
use std::intrinsics::transmute;

use rand::Rng;

use crate::move_::MoveCategory;

pub mod move_;
pub mod pokemon;
pub mod setup;
pub mod species;
pub mod state;

pub static mut GAME_VERSION: GameVersion = GameVersion::SS;

fn game_version() -> &'static GameVersion { unsafe { &GAME_VERSION } }

fn clamp<T: PartialOrd + Debug>(i: T, min: T, max: T) -> T {
    if min > max { panic!(format!("min must not be greater than max. (min, max): ({:?}, {:?})", min, max)) }
    if i < min { min } else if i > max { max } else { i }
}

fn choose_weighted_index(weights: &[f64]) -> usize {
    if weights.is_empty() || weights.iter().any(|d| !almost::zero(*d) && *d < 0.0) {
        panic!(format!("Weights must be non-negative. Given weights: {:?}", weights));
    }

    // TODO: Use seeded RNG
    let mut d = rand::thread_rng().gen_range::<f64, f64, f64>(0.0, weights.iter().sum());
    for i in 0..weights.len() {
        let w = *weights.get(i).unwrap();
        if d < w { return i; }
        d -= w;
    }
    weights.len() - 1
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum FieldPosition {
    Min,
    Max,
}

impl FieldPosition {
    const fn x(&self) -> i8 {
        match self {
            FieldPosition::Min => 0,
            FieldPosition::Max => 0
        }
    }

    const fn y(&self) -> i8 {
        match self {
            FieldPosition::Min => 0,
            FieldPosition::Max => 1
        }
    }

    const fn opposes(&self, other: FieldPosition) -> bool {
        self.y() != other.y()
    }

    fn adjacent_to(&self, other: FieldPosition) -> bool {
        max((other.x() - self.x()).abs(), (other.y() - self.y()).abs()) == 1
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Type {
    None = 0,
    Normal = 1,
    Fighting = 2,
    Flying = 3,
    Poison = 4,
    Ground = 5,
    Rock = 6,
    Bug = 7,
    Ghost = 8,
    Steel = 9,
    Fire = 10,
    Water = 11,
    Grass = 12,
    Electric = 13,
    Psychic = 14,
    Ice = 15,
    Dragon = 16,
    Dark = 17,
    Fairy = 18,
}

impl Type {
    const fn category(&self) -> MoveCategory {
        match self {
            Type::None | Type::Normal | Type::Fighting | Type::Flying | Type::Poison | Type::Ground | Type::Rock | Type::Bug | Type::Ghost | Type::Steel => MoveCategory::Physical,
            _ => MoveCategory::Special
        }
    }

    fn effectiveness(&self, defending_type1: Type, defending_type2: Type) -> f64 {
        self.effectiveness_single_type(defending_type1) * self.effectiveness_single_type(defending_type2)
    }

    fn effectiveness_single_type(&self, defending_type: Type) -> f64 {
        unsafe {
            match self {
                Type::None => 1.0,
                Type::Normal => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 0.0, 0.5, 1.0, 1.0, 1.0,
                    1.0, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Fighting => [1.0, 2.0, 1.0, 0.5, 0.5, 1.0, 2.0, 0.5, 0.0, 2.0, 1.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0, 2.0, 0.5][transmute::<Type, u8>(defending_type) as usize],
                Type::Flying => [1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0, 0.5, 1.0, 1.0, 2.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Poison => [1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 0.5, 0.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Ground => [1.0, 1.0, 1.0, 0.0, 2.0, 1.0, 2.0, 0.5, 1.0, 2.0, 2.0, 1.0, 0.5, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Rock => [1.0, 1.0, 0.5, 2.0, 1.0, 0.5, 1.0, 2.0, 1.0, 0.5, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Bug => [1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 2.0, 0.5][transmute::<Type, u8>(defending_type) as usize],
                Type::Ghost => [1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, if game_version().gen() <= 5 { 0.5 } else { 1.0 }, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Steel => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 0.5, 1.0, 2.0, 1.0, 1.0, 2.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Fire => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0, 2.0, 0.5, 0.5, 2.0, 1.0, 1.0, 2.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Water => [1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 1.0, 2.0, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Grass => [1.0, 1.0, 1.0, 0.5, 0.5, 2.0, 2.0, 0.5, 1.0, 0.5, 0.5, 2.0, 0.5, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Electric => [1.0, 1.0, 1.0, 2.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 0.5, 0.5, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Psychic => [1.0, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0, 0.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Ice => [1.0, 1.0, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 2.0, 1.0, 1.0, 0.5, 2.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Dragon => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 0.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Dark => [1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, if game_version().gen() <= 5 { 0.5 } else { 1.0 }, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 0.5][transmute::<Type, u8>(defending_type) as usize],
                Type::Fairy => [1.0, 1.0, 2.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0][transmute::<Type, u8>(defending_type) as usize]
            }
        }
    }
}

impl Default for Type {
    fn default() -> Self { Type::None }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Terrain {
    Normal,
    Electric,
    Grassy,
    Misty,
    Psychic,
}

impl Default for Terrain {
    fn default() -> Self { Terrain::Normal }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Weather {
    None,
    Sunshine,
    HarshSunshine,
    Rain,
    HeavyRain,
    Hail,
    Sandstorm,
    StrongWinds,
    Fog,
}

impl Default for Weather {
    fn default() -> Self { Weather::None }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Ability {
    None,
    Chlorophyll,
    Overgrow,
}

impl Default for Ability {
    fn default() -> Self { Ability::None }
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum GameVersion {
    RS,
    E,
    FRLG,
    DP,
    PT,
    HGSS,
    BW,
    B2W2,
    XY,
    ORAS,
    SM,
    USUM,
    LGPLGE,
    SS,
    PIXELMON(u32, u32), // Mod version
}

impl GameVersion {
    fn _filename(&self) -> String {
        match self {
            GameVersion::RS => String::from("ruby_sapphire"),
            GameVersion::E => String::from("emerald"),
            GameVersion::FRLG => String::from("firered_leafgreen"),
            GameVersion::DP => String::from("diamond_pearl"),
            GameVersion::PT => String::from("platinum"),
            GameVersion::HGSS => String::from("heartgold_soulsilver"),
            GameVersion::BW => String::from("black_white"),
            GameVersion::B2W2 => String::from("black2_white2"),
            GameVersion::XY => String::from("x_y"),
            GameVersion::ORAS => String::from("omegaruby_alphasapphire"),
            GameVersion::SM => String::from("sun_moon"),
            GameVersion::USUM => String::from("ultrasun_ultramoon"),
            GameVersion::LGPLGE => String::from("letsgopikachu_letsgoeevee"),
            GameVersion::SS => String::from("sword_shield"),
            GameVersion::PIXELMON(major, minor) => format!("pixelmon-{}.{}", major, minor)
        }
    }

    const fn gen(&self) -> u32 {
        match self {
            GameVersion::RS | GameVersion::E | GameVersion::FRLG => 3,
            GameVersion::DP | GameVersion::PT | GameVersion::HGSS => 4,
            GameVersion::BW | GameVersion::B2W2 => 5,
            GameVersion::XY | GameVersion::ORAS => 6,
            GameVersion::SM | GameVersion::USUM | GameVersion::LGPLGE => 7,
            GameVersion::SS => 8,
            GameVersion::PIXELMON(_major, _minor) => 8 // TODO: Put proper gens here
        }
    }
}

impl Default for GameVersion {
    fn default() -> Self { GameVersion::SS }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Gender {
    None,
    Male,
    Female,
}

impl Gender {
    const fn symbol(&self) -> &str {
        match self {
            Gender::None => "",
            Gender::Male => "♂",
            Gender::Female => "♀"
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MajorStatusAilment {
    Okay,
    Asleep,
    Poisoned,
    BadlyPoisoned,
    Paralyzed,
    Burned,
    Frozen,
}

impl MajorStatusAilment {
    const fn display_text_when_cured(&self) -> &'static str {
        match self {
            MajorStatusAilment::Okay => "",
            MajorStatusAilment::Asleep => " woke up!",
            MajorStatusAilment::Paralyzed => " was cured of its paralysis!",
            MajorStatusAilment::Burned => " was cured of its burn!",
            MajorStatusAilment::Frozen => " thawed out!",
            _ => " was cured of its poisoning!"
        }
    }
}

impl Default for MajorStatusAilment {
    fn default() -> Self { MajorStatusAilment::Okay }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum Nature {
    Adamant = 0,
    Bashful = 1,
    Bold = 2,
    Brave = 3,
    Calm = 4,
    Careful = 5,
    Docile = 6,
    Gentle = 7,
    Hardy = 8,
    Hasty = 9,
    Impish = 10,
    Jolly = 11,
    Lax = 12,
    Lonely = 13,
    Mild = 14,
    Modest = 15,
    Naive = 16,
    Naughty = 17,
    Quiet = 18,
    Quirky = 19,
    Rash = 20,
    Relaxed = 21,
    Sassy = 22,
    Serious = 23,
    Timid = 24,
}

impl Nature {
    pub fn random_nature() -> Nature {
        unsafe {
            // TODO: Use seeded RNG
            transmute(rand::thread_rng().gen_range::<u8, u8, u8>(0, 25))
        }
    }

    fn stat_mod(&self, stat_index: StatIndex) -> f64 {
        match stat_index {
            StatIndex::Hp | StatIndex::Acc | StatIndex::Eva => 1.0,
            _ => unsafe {
                match self {
                    Nature::Adamant => [1.1, 1.0, 0.9, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Bold => [0.9, 1.1, 1.0, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Brave => [1.1, 1.0, 1.0, 1.0, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Calm => [0.9, 1.0, 1.0, 1.1, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Careful => [1.0, 1.0, 0.9, 1.1, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Gentle => [1.0, 0.9, 1.0, 1.1, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Hasty => [1.0, 0.9, 1.0, 1.0, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Impish => [1.0, 1.1, 0.9, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Jolly => [1.0, 1.0, 0.9, 1.0, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Lax => [1.0, 1.1, 1.0, 0.9, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Lonely => [1.1, 0.9, 1.0, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Mild => [1.0, 0.9, 1.1, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Modest => [0.9, 1.0, 1.1, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Naive => [1.0, 1.0, 1.0, 0.9, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Naughty => [1.1, 1.0, 1.0, 0.9, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Quiet => [1.0, 1.0, 1.1, 1.0, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Rash => [1.0, 1.0, 1.1, 0.9, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Relaxed => [1.0, 1.1, 1.0, 1.0, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Sassy => [1.0, 1.0, 1.0, 1.1, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Timid => [0.9, 1.0, 1.0, 1.0, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    _ => 1.0
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(usize)]
pub enum StatIndex {
    Hp = 0,
    Atk = 1,
    Def = 2,
    SpAtk = 3,
    SpDef = 4,
    Spd = 5,
    Acc = 6,
    Eva = 7,
}

impl StatIndex {
    const fn name(&self) -> &str {
        match self {
            StatIndex::Hp => "HP",
            StatIndex::Atk => "attack",
            StatIndex::Def => "defense",
            StatIndex::SpAtk => "special attack",
            StatIndex::SpDef => "special defense",
            StatIndex::Spd => "speed",
            StatIndex::Acc => "accuracy",
            StatIndex::Eva => "evasion"
        }
    }

    fn as_usize(&self) -> usize {
        unsafe {
            transmute(*self)
        }
    }
}
