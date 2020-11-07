use std::cmp::max;
use std::fmt::Debug;
use std::intrinsics::transmute;

use rand::Rng;

use crate::move_::MoveCategory;
use rand::prelude::StdRng;

pub mod game_theory;
pub mod move_;
pub mod pokemon;
pub mod solution;
pub mod species;
pub mod state;

pub static mut GAME_VERSION: GameVersion = GameVersion::SS;

pub fn game_version() -> &'static GameVersion { unsafe { &GAME_VERSION } }

fn clamp<T: PartialOrd + Debug>(i: T, min: T, max: T) -> T {
    if min > max { panic!(format!("min must not be greater than max. (min, max): ({:?}, {:?})", min, max)) }
    if i < min { min } else if i > max { max } else { i }
}

fn choose_weighted_index(weights: &[f64], rng: &mut StdRng) -> usize {
    if weights.is_empty() || weights.iter().any(|d| !almost::zero(*d) && *d < 0.0) {
        panic!(format!("Weights must be non-negative. Given weights: {:?}", weights));
    }

    let mut d = rng.gen_range::<f64, f64, f64>(0.0, weights.iter().sum());
    for (i, &weight) in weights.iter().enumerate() {
        if d < weight { return i; }
        d -= weight;
    }
    weights.len() - 1
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FieldPosition {
    Min,
    Max
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Type {
    None,
    Normal,
    Fighting,
    Flying,
    Poison,
    Ground,
    Rock,
    Bug,
    Ghost,
    Steel,
    Fire,
    Water,
    Grass,
    Electric,
    Psychic,
    Ice,
    Dragon,
    Dark,
    Fairy
}

impl Type {
    fn by_name(name: &str) -> Result<Type, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "none"     => Ok(Type::None),
            "normal"   => Ok(Type::Normal),
            "fighting" => Ok(Type::Fighting),
            "flying"   => Ok(Type::Flying),
            "poison"   => Ok(Type::Poison),
            "ground"   => Ok(Type::Ground),
            "rock"     => Ok(Type::Rock),
            "bug"      => Ok(Type::Bug),
            "ghost"    => Ok(Type::Ghost),
            "steel"    => Ok(Type::Steel),
            "fire"     => Ok(Type::Fire),
            "water"    => Ok(Type::Water),
            "grass"    => Ok(Type::Grass),
            "electric" => Ok(Type::Electric),
            "psychic"  => Ok(Type::Psychic),
            "ice"      => Ok(Type::Ice),
            "dragon"   => Ok(Type::Dragon),
            "dark"     => Ok(Type::Dark),
            "fairy"    => Ok(Type::Fairy),
            _ => Err(format!("Invalid type '{}'", name))
        }
    }

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
                Type::Normal   => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 0.0,                                               0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Fighting => [1.0, 2.0, 1.0, 0.5, 0.5, 1.0, 2.0, 0.5, 0.0,                                               2.0, 1.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0, 2.0, 0.5][transmute::<Type, u8>(defending_type) as usize],
                Type::Flying   => [1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0,                                               0.5, 1.0, 1.0, 2.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Poison   => [1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 0.5,                                               0.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Ground   => [1.0, 1.0, 1.0, 0.0, 2.0, 1.0, 2.0, 0.5, 1.0,                                               2.0, 2.0, 1.0, 0.5, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Rock     => [1.0, 1.0, 0.5, 2.0, 1.0, 0.5, 1.0, 2.0, 1.0,                                               0.5, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Bug      => [1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5,                                               0.5, 0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 2.0, 0.5][transmute::<Type, u8>(defending_type) as usize],
                Type::Ghost    => [1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, if game_version().gen() <= 5 { 0.5 } else { 1.0 }, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Steel    => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0,                                               0.5, 0.5, 0.5, 1.0, 0.5, 1.0, 2.0, 1.0, 1.0, 2.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Fire     => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0,                                               2.0, 0.5, 0.5, 2.0, 1.0, 1.0, 2.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Water    => [1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0,                                               1.0, 2.0, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Grass    => [1.0, 1.0, 1.0, 0.5, 0.5, 2.0, 2.0, 0.5, 1.0,                                               0.5, 0.5, 2.0, 0.5, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Electric => [1.0, 1.0, 1.0, 2.0, 1.0, 0.0, 1.0, 1.0, 1.0,                                               1.0, 1.0, 2.0, 0.5, 0.5, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Psychic  => [1.0, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0,                                               0.5, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0, 0.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Ice      => [1.0, 1.0, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0,                                               0.5, 0.5, 0.5, 2.0, 1.0, 1.0, 0.5, 2.0, 1.0, 1.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Dragon   => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,                                               0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 0.0][transmute::<Type, u8>(defending_type) as usize],
                Type::Dark     => [1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, if game_version().gen() <= 5 { 0.5 } else { 1.0 }, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 0.5][transmute::<Type, u8>(defending_type) as usize],
                Type::Fairy    => [1.0, 1.0, 2.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0,                                               0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0][transmute::<Type, u8>(defending_type) as usize]
            }
        }
    }
}

impl Default for Type {
    fn default() -> Self { Type::None }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Terrain {
    Normal,
    Electric,
    Grassy,
    Misty,
    Psychic
}

impl Default for Terrain {
    fn default() -> Self { Terrain::Normal }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Weather {
    None,
    Sunshine,
    HarshSunshine,
    Rain,
    HeavyRain,
    Hail,
    Sandstorm,
    StrongWinds,
    Fog
}

impl Default for Weather {
    fn default() -> Self { Weather::None }
}

type AbilityID = u8;

struct Ability {
    name: &'static str
}

impl Ability {
    fn id_by_name(name: &str) -> Result<AbilityID, String> {
        for (ability_id, ability) in ABILITIES.iter().enumerate() {
            if ability.name.eq_ignore_ascii_case(name) {
                return Ok(ability_id as AbilityID);
            }
        }
        Err(format!("Invalid ability '{}'", name))
    }
}

const ABILITIES: [Ability; 3] = [
    Ability { name: "None" },
    Ability { name: "Chlorophyll" },
    Ability { name: "Overgrow" }
];

#[derive(Debug, Eq, PartialEq)]
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
    SS
}

impl GameVersion {
    const fn name(&self) -> &'static str {
        match self {
            GameVersion::RS => "ruby_sapphire",
            GameVersion::E => "emerald",
            GameVersion::FRLG => "firered_leafgreen",
            GameVersion::DP => "diamond_pearl",
            GameVersion::PT => "platinum",
            GameVersion::HGSS => "heartgold_soulsilver",
            GameVersion::BW => "black_white",
            GameVersion::B2W2 => "black2_white2",
            GameVersion::XY => "x_y",
            GameVersion::ORAS => "omegaruby_alphasapphire",
            GameVersion::SM => "sun_moon",
            GameVersion::USUM => "ultrasun_ultramoon",
            GameVersion::LGPLGE => "letsgopikachu_letsgoeevee",
            GameVersion::SS => "sword_shield"
        }
    }

    const fn gen(&self) -> u32 {
        match self {
            GameVersion::RS | GameVersion::E | GameVersion::FRLG => 3,
            GameVersion::DP | GameVersion::PT | GameVersion::HGSS => 4,
            GameVersion::BW | GameVersion::B2W2 => 5,
            GameVersion::XY | GameVersion::ORAS => 6,
            GameVersion::SM | GameVersion::USUM | GameVersion::LGPLGE => 7,
            GameVersion::SS => 8
        }
    }
}

impl Default for GameVersion {
    fn default() -> Self { GameVersion::SS }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Gender {
    None,
    Male,
    Female
}

impl Gender {
    fn by_name(name: &str) -> Result<Gender, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "none" => Ok(Gender::None),
            "male" => Ok(Gender::Male),
            "female" => Ok(Gender::Female),
            _ => Err(format!("Invalid gender '{}'", name))
        }
    }

    const fn _name(&self) -> &'static str {
        match self {
            Gender::None => "None",
            Gender::Male => "Male",
            Gender::Female => "Female"
        }
    }

    const fn symbol(&self) -> &'static str {
        match self {
            Gender::None => "",
            Gender::Male => "♂",
            Gender::Female => "♀"
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MajorStatusAilment {
    Okay,
    Asleep,
    Poisoned,
    BadlyPoisoned,
    Paralyzed,
    Burned,
    Frozen
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

    const fn display_text_when_blocking_move(&self) -> &'static str {
        match self {
            MajorStatusAilment::Asleep => " is fast asleep.",
            MajorStatusAilment::Paralyzed => " is paralyzed! It can't move!",
            MajorStatusAilment::Frozen => " is frozen solid!",
            _ => ""
        }
    }
}

impl Default for MajorStatusAilment {
    fn default() -> Self { MajorStatusAilment::Okay }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Nature {
    Adamant,
    Bashful,
    Bold,
    Brave,
    Calm,
    Careful,
    Docile,
    Gentle,
    Hardy,
    Hasty,
    Impish,
    Jolly,
    Lax,
    Lonely,
    Mild,
    Modest,
    Naive,
    Naughty,
    Quiet,
    Quirky,
    Rash,
    Relaxed,
    Sassy,
    Serious,
    Timid
}

impl Nature {
    pub fn random_nature(rng: &mut StdRng) -> Nature {
        unsafe {
            transmute::<u8, Nature>(rng.gen_range(0, 25))
        }
    }

    fn by_name(name: &str) -> Result<Nature, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "adamant" => Ok(Nature::Adamant),
            "bashful" => Ok(Nature::Bashful),
            "bold"    => Ok(Nature::Bold),
            "brave"   => Ok(Nature::Brave),
            "calm"    => Ok(Nature::Calm),
            "careful" => Ok(Nature::Careful),
            "docile"  => Ok(Nature::Docile),
            "gentle"  => Ok(Nature::Gentle),
            "hardy"   => Ok(Nature::Hardy),
            "hasty"   => Ok(Nature::Hasty),
            "impish"  => Ok(Nature::Impish),
            "jolly"   => Ok(Nature::Jolly),
            "lax"     => Ok(Nature::Lax),
            "lonely"  => Ok(Nature::Lonely),
            "mild"    => Ok(Nature::Mild),
            "modest"  => Ok(Nature::Modest),
            "naive"   => Ok(Nature::Naive),
            "naughty" => Ok(Nature::Naughty),
            "quiet"   => Ok(Nature::Quiet),
            "quirky"  => Ok(Nature::Quirky),
            "rash"    => Ok(Nature::Rash),
            "relaxed" => Ok(Nature::Relaxed),
            "sassy"   => Ok(Nature::Sassy),
            "serious" => Ok(Nature::Serious),
            "timid"   => Ok(Nature::Timid),
            _ => Err(format!("Invalid nature '{}'", name))
        }
    }

    const fn stat_mod(&self, stat_index: StatIndex) -> f64 {
        match stat_index {
            StatIndex::Hp | StatIndex::Acc | StatIndex::Eva => 1.0,
            _ => match self {
                Nature::Adamant => [1.1, 1.0, 0.9, 1.0, 1.0][stat_index.as_usize() - 1],
                Nature::Bold    => [0.9, 1.1, 1.0, 1.0, 1.0][stat_index.as_usize() - 1],
                Nature::Brave   => [1.1, 1.0, 1.0, 1.0, 0.9][stat_index.as_usize() - 1],
                Nature::Calm    => [0.9, 1.0, 1.0, 1.1, 1.0][stat_index.as_usize() - 1],
                Nature::Careful => [1.0, 1.0, 0.9, 1.1, 1.0][stat_index.as_usize() - 1],
                Nature::Gentle  => [1.0, 0.9, 1.0, 1.1, 1.0][stat_index.as_usize() - 1],
                Nature::Hasty   => [1.0, 0.9, 1.0, 1.0, 1.1][stat_index.as_usize() - 1],
                Nature::Impish  => [1.0, 1.1, 0.9, 1.0, 1.0][stat_index.as_usize() - 1],
                Nature::Jolly   => [1.0, 1.0, 0.9, 1.0, 1.1][stat_index.as_usize() - 1],
                Nature::Lax     => [1.0, 1.1, 1.0, 0.9, 1.0][stat_index.as_usize() - 1],
                Nature::Lonely  => [1.1, 0.9, 1.0, 1.0, 1.0][stat_index.as_usize() - 1],
                Nature::Mild    => [1.0, 0.9, 1.1, 1.0, 1.0][stat_index.as_usize() - 1],
                Nature::Modest  => [0.9, 1.0, 1.1, 1.0, 1.0][stat_index.as_usize() - 1],
                Nature::Naive   => [1.0, 1.0, 1.0, 0.9, 1.1][stat_index.as_usize() - 1],
                Nature::Naughty => [1.1, 1.0, 1.0, 0.9, 1.0][stat_index.as_usize() - 1],
                Nature::Quiet   => [1.0, 1.0, 1.1, 1.0, 0.9][stat_index.as_usize() - 1],
                Nature::Rash    => [1.0, 1.0, 1.1, 0.9, 1.0][stat_index.as_usize() - 1],
                Nature::Relaxed => [1.0, 1.1, 1.0, 1.0, 0.9][stat_index.as_usize() - 1],
                Nature::Sassy   => [1.0, 1.0, 1.0, 1.1, 0.9][stat_index.as_usize() - 1],
                Nature::Timid   => [0.9, 1.0, 1.0, 1.0, 1.1][stat_index.as_usize() - 1],
                _ => 1.0
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(usize)]
pub enum StatIndex {
    Hp,
    Atk,
    Def,
    SpAtk,
    SpDef,
    Spd,
    Acc,
    Eva
}

impl StatIndex {
    fn by_name(name: &str) -> Result<StatIndex, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "hp"    => Ok(StatIndex::Hp),
            "atk"   => Ok(StatIndex::Atk),
            "def"   => Ok(StatIndex::Def),
            "spatk" => Ok(StatIndex::SpAtk),
            "spdef" => Ok(StatIndex::SpDef),
            "spd"   => Ok(StatIndex::Spd),
            "acc"   => Ok(StatIndex::Acc),
            "eva"   => Ok(StatIndex::Eva),
            _ => Err(format!("Invalid stat index '{}'", name))
        }
    }

    const fn name(&self) -> &'static str {
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

    const fn as_usize(&self) -> usize {
        match self {
            StatIndex::Hp => 0,
            StatIndex::Atk => 1,
            StatIndex::Def => 2,
            StatIndex::SpAtk => 3,
            StatIndex::SpDef => 4,
            StatIndex::Spd => 5,
            StatIndex::Acc => 6,
            StatIndex::Eva => 7
        }
    }
}
