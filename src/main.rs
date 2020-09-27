extern crate strum;
extern crate strum_macros;

use crate::{move_::{MoveCategory, MoveAction}, species::Species, state::State};
use rand::Rng;
use std::{env, intrinsics::transmute};
use std::cmp::max;
use std::fmt::{Display, Debug, Error, Formatter};
use strum_macros::Display;
use crate::move_::Move;

mod move_;
mod species;
mod state;

static mut GAME_VERSION: GameVersion = GameVersion::SS;
fn game_version() -> &'static GameVersion { unsafe { &GAME_VERSION } }

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("Args: {:?}", args);

    // TODO: Parse game version from args
    unsafe {
        GAME_VERSION = GameVersion::XY;
    }
}

fn clamp<T: PartialOrd + Debug>(i: T, min: T, max: T) -> T {
    if !(min <= max) { panic!(format!("min must not be greater than max. (min, max): ({:?}, {:?})", min, max)) }
    if i < min { min } else if i > max { max } else { i }
}

fn choose_weighted_index(weights: &[f64]) -> usize {
    if weights.iter().any(|d| !almost::zero(*d) && *d < 0.0) {
        panic!(format!("Weights must be non-negative. Given weights: {:?}", weights));
    }

    // TODO: Use seeded RNG
    let mut d = rand::thread_rng().gen_range::<f64, f64, f64>(0.0, weights.iter().sum());
    for i in 0..weights.len() {
        let w = *weights.get(i).unwrap();
        if d < w { return i; }
        d -= w;
    }
    return weights.len() - 1;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum FieldPosition {
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(usize)]
enum Type {
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
    Fairy = 18
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
                Type::None     => 1.0,
                Type::Normal   => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 0.0,                                               0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Fighting => [1.0, 2.0, 1.0, 0.5, 0.5, 1.0, 2.0, 0.5, 0.0,                                               2.0, 1.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0, 2.0, 0.5][transmute::<Type, usize>(defending_type)],
                Type::Flying   => [1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0,                                               0.5, 1.0, 1.0, 2.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Poison   => [1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 0.5,                                               0.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0][transmute::<Type, usize>(defending_type)],
                Type::Ground   => [1.0, 1.0, 1.0, 0.0, 2.0, 1.0, 2.0, 0.5, 1.0,                                               2.0, 2.0, 1.0, 0.5, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Rock     => [1.0, 1.0, 0.5, 2.0, 1.0, 0.5, 1.0, 2.0, 1.0,                                               0.5, 2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Bug      => [1.0, 1.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5,                                               0.5, 0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 2.0, 0.5][transmute::<Type, usize>(defending_type)],
                Type::Ghost    => [1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, if game_version().gen() <= 5 { 0.5 } else { 1.0 }, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Steel    => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0,                                               0.5, 0.5, 0.5, 1.0, 0.5, 1.0, 2.0, 1.0, 1.0, 2.0][transmute::<Type, usize>(defending_type)],
                Type::Fire     => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 2.0, 1.0,                                               2.0, 0.5, 0.5, 2.0, 1.0, 1.0, 2.0, 0.5, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Water    => [1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0,                                               1.0, 2.0, 0.5, 0.5, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Grass    => [1.0, 1.0, 1.0, 0.5, 0.5, 2.0, 2.0, 0.5, 1.0,                                               0.5, 0.5, 2.0, 0.5, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Electric => [1.0, 1.0, 1.0, 2.0, 1.0, 0.0, 1.0, 1.0, 1.0,                                               1.0, 1.0, 2.0, 0.5, 0.5, 1.0, 1.0, 0.5, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Psychic  => [1.0, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0,                                               0.5, 1.0, 1.0, 1.0, 1.0, 0.5, 1.0, 1.0, 0.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Ice      => [1.0, 1.0, 1.0, 2.0, 1.0, 2.0, 1.0, 1.0, 1.0,                                               0.5, 0.5, 0.5, 2.0, 1.0, 1.0, 0.5, 2.0, 1.0, 1.0][transmute::<Type, usize>(defending_type)],
                Type::Dragon   => [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,                                               0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 0.0][transmute::<Type, usize>(defending_type)],
                Type::Dark     => [1.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, if game_version().gen() <= 5 { 0.5 } else { 1.0 }, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 0.5, 0.5][transmute::<Type, usize>(defending_type)],
                Type::Fairy    => [1.0, 1.0, 2.0, 1.0, 0.5, 1.0, 1.0, 1.0, 1.0,                                               0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 1.0][transmute::<Type, usize>(defending_type)]
            }
        }
    }
}

impl Default for Type {
    fn default() -> Self { Type::None }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Terrain {
    Normal,
    Electric,
    Grassy,
    Misty,
    Psychic
}

impl Default for Terrain {
    fn default() -> Self { Terrain::Normal }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Weather {
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Ability {
    None,
    Chlorophyll,
    Overgrow
}

impl Default for Ability {
    fn default() -> Self { Ability::None }
}

#[derive(Debug, Eq, PartialEq, Hash)]
enum GameVersion {
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
    PIXELMON(u32, u32) // Mod version
}

impl GameVersion {
    fn filename(&self) -> String {
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
            GameVersion::PIXELMON(major, minor) => 8 // TODO: Put proper gens here
        }
    }
}

impl Default for GameVersion {
    fn default() -> Self { GameVersion::SS }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum Gender {
    None,
    Male,
    Female
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum MajorStatusAilment {
    Okay,
    Asleep,
    Poisoned,
    BadlyPoisoned,
    Paralyzed,
    Burned,
    Frozen
}

impl MajorStatusAilment {
    const fn display_text_when_cured(&self) -> &str {
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
enum Nature {
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
    Timid = 24
}

impl Nature {
    fn random_nature() -> Nature {
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
                    Nature::Bold    => [0.9, 1.1, 1.0, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Brave   => [1.1, 1.0, 1.0, 1.0, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Calm    => [0.9, 1.0, 1.0, 1.1, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Careful => [1.0, 1.0, 0.9, 1.1, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Gentle  => [1.0, 0.9, 1.0, 1.1, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Hasty   => [1.0, 0.9, 1.0, 1.0, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Impish  => [1.0, 1.1, 0.9, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Jolly   => [1.0, 1.0, 0.9, 1.0, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Lax     => [1.0, 1.1, 1.0, 0.9, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Lonely  => [1.1, 0.9, 1.0, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Mild    => [1.0, 0.9, 1.1, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Modest  => [0.9, 1.0, 1.1, 1.0, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Naive   => [1.0, 1.0, 1.0, 0.9, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Naughty => [1.1, 1.0, 1.0, 0.9, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Quiet   => [1.0, 1.0, 1.1, 1.0, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Rash    => [1.0, 1.0, 1.1, 0.9, 1.0][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Relaxed => [1.0, 1.1, 1.0, 1.0, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Sassy   => [1.0, 1.0, 1.0, 1.1, 0.9][transmute::<StatIndex, usize>(stat_index) - 1],
                    Nature::Timid   => [0.9, 1.0, 1.0, 1.0, 1.1][transmute::<StatIndex, usize>(stat_index) - 1],
                    _ => 1.0
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(usize)]
enum StatIndex {
    Hp = 0,
    Atk = 1,
    Def = 2,
    SpAtk = 3,
    SpDef = 4,
    Spd = 5,
    Acc = 6,
    Eva = 7
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
            transmute(self)
        }
    }
}

#[derive(Clone, Debug)]
struct Pokemon {
    id: u8, // A unique identifier used throughout the program that persists when an instance is cloned
    species: &'static Species,
    first_type: Type, // Types usually match the species type, but some Pokemon can change types
    second_type: Type,
    gender: Gender,
    nature: Nature,
    ability: Ability,
    ivs: [u8; 6],
    evs: [u8; 6],
    max_hp: u16,
    current_hp: u16,
    stat_stages: [i8; 8],

    // Major status ailment
    major_status_ailment: MajorStatusAilment,
    msa_counter: u32,
    msa_counter_target: Option<u32>,
    snore_sleep_talk_counter: u32, // Only used in gen 3; otherwise it's always 0

    // Minor status ailments
    confusion_turn_inflicted: Option<u32>, // Turn on which confusion was inflicted
    confusion_turn_will_cure: Option<u32>, // Turn on which confusion will be cured naturally
    is_flinching: bool,
    seeded_by: Option<u8>, // ID of the Pokemon that seeded this Pokemon
    is_infatuated: bool,
    is_cursed: bool,
    has_nightmare: bool,

    field_position: Option<FieldPosition>,

    // Moves
    known_moves: Vec<&'static Move>, // TODO: Number of known moves can change during battle; should probably make a "MoveInstance" struct
    move_pp: Vec<u8>,
    move_disabled: Vec<bool>,
    next_move_action: Option<MoveAction> // Needed for handling two-turn moves
}

impl Pokemon {
    fn new(id: u8, species: &'static Species, gender: Gender, nature: Nature, ability: Ability, ivs: [u8; 6], evs: [u8; 6], known_moves: Vec<&'static Move>) -> Pokemon {
        let max_hp = (2 * species.base_stat(StatIndex::Hp) + ivs[StatIndex::Hp.as_usize()] + evs[StatIndex::Hp.as_usize()] / 4 + 110) as u16;
        Pokemon {
            id,
            species,
            first_type: species.first_type,
            second_type: species.second_type,
            gender,
            nature,
            ability,
            ivs,
            evs,
            max_hp,
            current_hp: max_hp,
            stat_stages: [0; 8],
            major_status_ailment: MajorStatusAilment::Okay,
            msa_counter: 0,
            msa_counter_target: None,
            snore_sleep_talk_counter: 0,
            confusion_turn_inflicted: None,
            confusion_turn_will_cure: None,
            is_flinching: false,
            seeded_by: None,
            is_infatuated: false,
            is_cursed: false,
            has_nightmare: false,
            field_position: None,
            move_pp: known_moves.iter().map(|move_| move_.max_pp).collect(),
            move_disabled: vec![false; known_moves.len()],
            known_moves,
            next_move_action: None
        }
    }

    fn calculated_stat(&self, state: &State, stat_index: StatIndex) -> u32 {
        if stat_index == StatIndex::Hp { return self.max_hp as u32; }

        let b = self.species.base_stat(stat_index) as u32;
        let i = self.ivs[stat_index.as_usize()] as u32;
        let e = self.evs[stat_index.as_usize()] as u32;
        let mut calculated_stat = ((2 * b + i + e / 4 + 5) as f64 * self.nature.stat_mod(stat_index)) as u32;

        if stat_index == StatIndex::Spd {
            if self.major_status_ailment == MajorStatusAilment::Paralyzed {
                calculated_stat /= if game_version().gen() <= 6 { 4 } else { 2 };
            }
            if self.ability == Ability::Chlorophyll && state.weather == Weather::Sunshine { calculated_stat *= 2; }
        }

        calculated_stat
    }

    fn is_type(&self, type_: Type) -> bool {
        self.first_type == type_ || self.second_type == type_
    }

    // TODO: Can move_index be usize instead of Option?
    // TODO: Pass a MoveInstance instead
    fn can_choose_move(&self, move_index: Option<usize>) -> bool {
        if self.current_hp == 0 || self.field_position == None { return false; }
        match move_index {
            Some(move_index) => self.move_pp[move_index] > 0 && !self.move_disabled[move_index],
            None => true
        }
    }

    fn add_to_field(&mut self, state: &mut State, field_position: FieldPosition) -> bool {
        state.display_text.push(format!("Adding {} to field position {:?}.", self, field_position));
        self.field_position = Some(field_position);
        match field_position {
            FieldPosition::Min => {
                match state.min_pokemon_id {
                    None => { state.min_pokemon_id = Some(self.id); },
                    Some(min_pokemon_id) => {
                        panic!(format!("Tried to add {} to position {:?} occupied by {}", self, field_position, state.pokemon_by_id(min_pokemon_id)));
                    }
                }
            },
            FieldPosition::Max => {
                match state.max_pokemon_id {
                    None => { state.max_pokemon_id = Some(self.id); },
                    Some(max_pokemon_id) => {
                        panic!(format!("Tried to add {} to position {:?} occupied by {}", self, field_position, state.pokemon_by_id(max_pokemon_id)));
                    }
                }
            }
        }

        state.battle_end_check()
    }

    fn remove_from_field(&mut self, state: &mut State) {
        state.display_text.push(format!("Removing {} from field position {:?}.", self, self.field_position));
        self.stat_stages = [0; 8];
        self.remove_minor_status_ailments();
        if game_version().gen() == 3 { self.snore_sleep_talk_counter = 0; }
        if game_version().gen() == 5 && self.major_status_ailment == MajorStatusAilment::Asleep { self.msa_counter = 0; }
        self.field_position = None;
        for i in 0..self.move_disabled.len() {
            self.move_disabled[i] = false;
        }
        self.next_move_action = None;
        let min_pokemon: Option<&mut Pokemon> = state.min_pokemon_id.map(|id| state.pokemon_by_id_mut(id));
        if let Some(min_pokemon) = min_pokemon {
            if let Some(seeder_id) = min_pokemon.seeded_by {
                if seeder_id == self.id { min_pokemon.seeded_by = None; }
            }
        }
        let max_pokemon: Option<&mut Pokemon> = state.max_pokemon_id.map(|id| state.pokemon_by_id_mut(id));
        if let Some(max_pokemon) = max_pokemon {
            if let Some(seeder_id) = max_pokemon.seeded_by {
                if seeder_id == self.id { max_pokemon.seeded_by = None; }
            }
        }

        if state.min_pokemon_id == Some(self.id) {
            state.min_pokemon_id = None;
        } else if state.max_pokemon_id == Some(self.id) {
            state.max_pokemon_id = None;
        } else {
            panic!(format!("ID of {} does not match any ID on the field.", self));
        }
    }

    fn stat_stage(&self, stat_index: StatIndex) -> i8 {
        self.stat_stages[stat_index.as_usize()]
    }

    fn increment_stat_stage(&mut self, state: &mut State, stat_index: StatIndex, requested_amount: i8) {
        let old_stat_stage = self.stat_stages[stat_index.as_usize()];
        let new_stat_stage = clamp(old_stat_stage + requested_amount, -6, 6);
        self.stat_stages[stat_index.as_usize()] = new_stat_stage;
        let actual_change = new_stat_stage - old_stat_stage;
        if actual_change <= -3 {
            state.display_text.push(format!("{}'s {} severely fell!", self.species.name, stat_index.name()));
        } else if actual_change == -2 {
            state.display_text.push(format!("{}'s {} harshly fell!", self.species.name, stat_index.name()));
        } else if actual_change == -1 {
            state.display_text.push(format!("{}'s {} fell!", self.species.name, stat_index.name()));
        } else if actual_change == 0 {
            state.display_text.push(format!("{}'s {} won't go any {}!", self.species.name, stat_index.name(), if requested_amount < 0 { "lower" } else { "higher" }));
        } else if actual_change == 1 {
            state.display_text.push(format!("{}'s {} rose!", self.species.name, stat_index.name()));
        } else if actual_change == 2 {
            state.display_text.push(format!("{}'s {} rose sharply!", self.species.name, stat_index.name()));
        } else {
            state.display_text.push(format!("{}'s {} rose drastically!", self.species.name, stat_index.name()));
        }
    }

    const fn major_status_ailment(&self) -> MajorStatusAilment {
        self.major_status_ailment
    }

    fn increment_msa_counter(&mut self, state: &mut State) {
        if let Some(msa_counter_target) = self.msa_counter_target {
            if self.major_status_ailment != MajorStatusAilment::Okay {
                self.msa_counter += self.snore_sleep_talk_counter + 1;
                self.snore_sleep_talk_counter = 0;
                if self.msa_counter >= msa_counter_target {
                    state.display_text.push(format!("{}{}", self.species.name, self.major_status_ailment.display_text_when_cured()));
                    self.major_status_ailment = MajorStatusAilment::Okay;
                    self.msa_counter = 0;
                    self.msa_counter_target = None;
                }
            }
        }
    }

    fn increment_snore_sleep_talk_counter(&mut self, game_version: &GameVersion, state: &mut State) {
        if self.major_status_ailment != MajorStatusAilment::Asleep { panic!("snore_sleep_talk_counter incremented while not asleep"); }
        if let Some(msa_counter_target) = self.msa_counter_target {
            if game_version.gen() == 3 {
                self.snore_sleep_talk_counter += 1;
                if self.snore_sleep_talk_counter >= msa_counter_target {
                    state.display_text.push(format!("{}{}", self.species.name, MajorStatusAilment::Asleep.display_text_when_cured()));
                    self.major_status_ailment = MajorStatusAilment::Okay;
                    self.snore_sleep_talk_counter = 0;
                    self.msa_counter = 0;
                    self.msa_counter_target = None;
                }
            }
        }
    }

    /// Returns true if the poisoning was successful.
    fn inflict_poison(&mut self, state: &mut State) -> bool {
        if self.major_status_ailment == MajorStatusAilment::Okay && !self.is_type(Type::Poison) && !self.is_type(Type::Steel) {
            self.major_status_ailment = MajorStatusAilment::Poisoned;
            self.msa_counter = 0;
            self.msa_counter_target = None;
            state.display_text.push(format!("{} was poisoned!", self.species.name));
            true
        } else {
            false
        }
    }

    /// Returns true if putting this Pokemon to sleep was successful.
    fn inflict_sleep(&mut self, state: &mut State) -> bool {
        if self.major_status_ailment == MajorStatusAilment::Okay {
            self.major_status_ailment = MajorStatusAilment::Asleep;
            self.msa_counter = 0;
            // TODO: Use seeded RNG
            self.msa_counter_target = Some(if game_version().gen() <= 4 { rand::thread_rng().gen_range(2, 6) } else { rand::thread_rng().gen_range(1, 4) });
            if game_version().gen() == 3 { self.snore_sleep_talk_counter = 0; }
            state.display_text.push(format!("{} fell asleep!", self.species.name));
            true
        } else {
            false
        }
    }

    /// The amount can be negative to add HP.
    fn apply_damage(&mut self, state: &mut State, amount: i16) -> bool {
        let new_hp = self.current_hp as i16 - amount;
        if new_hp <= 0 {
            self.current_hp = 0;
            state.display_text.push(format!("{} fainted!", self));
            self.remove_from_field(state);
            return state.battle_end_check();
        }
        self.current_hp = new_hp as u16;
        if self.current_hp > self.max_hp {
            self.current_hp = self.max_hp;
        }
        false
    }

    fn remove_minor_status_ailments(&mut self) {
        self.confusion_turn_inflicted = None;
        self.confusion_turn_will_cure = None;
        self.is_flinching = false;
        self.seeded_by = None;
        self.is_infatuated = false;
        self.is_cursed = false;
        self.has_nightmare = false;
    }
}

impl Display for Pokemon {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}{}(ID = {}, HP = {}/{})", self.species.name, self.gender.symbol(), self.id, self.current_hp, self.max_hp)
    }
}
