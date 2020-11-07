use std::cmp::{max, min};
use std::fmt::{Debug, Error, Formatter};
use std::fs;
use std::process::exit;

use rand::Rng;
use rand::prelude::StdRng;

use crate::{Ability, FieldPosition, game_version, MajorStatusAilment, pokemon, StatIndex, Type, clamp};
use crate::state::State;
use crate::species::Species;
use json::JsonValue;
use json::number::Number;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MoveCategory {
    Physical,
    Special,
    Status
}

impl MoveCategory {
    fn by_name(name: &str) -> Result<MoveCategory, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "physical" => Ok(MoveCategory::Physical),
            "special"  => Ok(MoveCategory::Special),
            "status"   => Ok(MoveCategory::Status),
            _ => Err(format!("Invalid move category '{}'", name))
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MoveTargeting {
    RandomOpponent,
    SingleAdjacentAlly,
    SingleAdjacentOpponent,
    SingleAdjacentPokemon,
    SinglePokemon,
    User,
    UserOrAdjacentAlly,
    UserAndAllAllies,
    AllAdjacentOpponents,
    AllAdjacentPokemon,
    AllAllies,
    AllOpponents,
    AllPokemon
}

impl MoveTargeting {
    fn by_name(name: &str) -> Result<MoveTargeting, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "randomopponent"         => Ok(MoveTargeting::RandomOpponent),
            "singleadjacentally"     => Ok(MoveTargeting::SingleAdjacentAlly),
            "singleadjacentopponent" => Ok(MoveTargeting::SingleAdjacentOpponent),
            "singleadjacentpokemon"  => Ok(MoveTargeting::SingleAdjacentPokemon),
            "singlepokemon"          => Ok(MoveTargeting::SinglePokemon),
            "user"                   => Ok(MoveTargeting::User),
            "useroradjacentally"     => Ok(MoveTargeting::UserOrAdjacentAlly),
            "userandallallies"       => Ok(MoveTargeting::UserAndAllAllies),
            "alladjacentopponents"   => Ok(MoveTargeting::AllAdjacentOpponents),
            "alladjacentpokemon"     => Ok(MoveTargeting::AllAdjacentPokemon),
            "allallies"              => Ok(MoveTargeting::AllAllies),
            "allopponents"           => Ok(MoveTargeting::AllOpponents),
            "allpokemon"             => Ok(MoveTargeting::AllPokemon),
            _ => Err(format!("Invalid move targeting '{}'", name))
        }
    }

    const fn single_target(&self) -> bool {
        matches!(self, MoveTargeting::RandomOpponent
                     | MoveTargeting::SingleAdjacentAlly
                     | MoveTargeting::SingleAdjacentOpponent
                     | MoveTargeting::SingleAdjacentPokemon
                     | MoveTargeting::SinglePokemon
                     | MoveTargeting::User
                     | MoveTargeting::UserOrAdjacentAlly)
    }

    const fn only_targets_allies(&self) -> bool {
        matches!(self, MoveTargeting::SingleAdjacentAlly
                     | MoveTargeting::User
                     | MoveTargeting::UserOrAdjacentAlly
                     | MoveTargeting::UserAndAllAllies
                     | MoveTargeting::AllAllies)
    }

    pub fn can_hit(&self, user_pos: FieldPosition, target_pos: FieldPosition) -> bool {
        match self {
            MoveTargeting::RandomOpponent | MoveTargeting::AllOpponents => user_pos.opposes(target_pos),
            MoveTargeting::SingleAdjacentAlly => !user_pos.opposes(target_pos) && user_pos.adjacent_to(target_pos),
            MoveTargeting::SingleAdjacentOpponent | MoveTargeting::AllAdjacentOpponents => user_pos.opposes(target_pos) && user_pos.adjacent_to(target_pos),
            MoveTargeting::SingleAdjacentPokemon | MoveTargeting::AllAdjacentPokemon => user_pos.adjacent_to(target_pos),
            MoveTargeting::SinglePokemon => user_pos != target_pos,
            MoveTargeting::User => user_pos == target_pos,
            MoveTargeting::UserOrAdjacentAlly => MoveTargeting::User.can_hit(user_pos, target_pos) || MoveTargeting::SingleAdjacentAlly.can_hit(user_pos, target_pos),
            MoveTargeting::UserAndAllAllies => !user_pos.opposes(target_pos),
            MoveTargeting::AllAllies => user_pos != target_pos && !user_pos.opposes(target_pos),
            MoveTargeting::AllPokemon => true
        }
    }
}

/// An action selection that will be queued and executed during a turn.
#[derive(Clone, Debug)]
pub enum Action {
    /// An action where the user performs one of its known moves.
    Move {
        user_id: u8,
        move_: MoveID,
        move_index: Option<u8>,
        target_positions: Vec<FieldPosition>
    },
    /// An action where the user switches places with a team member not currently on the field.
    Switch {
        user_id: u8,
        switching_in_id: u8
    }
}

impl Action {
    /**
     * @param state - a game state
     * @param otherAction - some other move action
     * @return Whether this move action should come before {@code otherAction} based on priority and the user's speed.
     */
    pub fn outspeeds(&self, state_box: &State, other_action: &Action, rng: &mut StdRng) -> bool {
        match other_action {
            Action::Switch {user_id: _, switching_in_id: _} => false,
            Action::Move {user_id: other_user_id, move_: other_move, move_index: _, target_positions: _} => {
                match self {
                    Action::Switch {user_id: _, switching_in_id: _} => true,
                    Action::Move {user_id, move_, move_index: _, target_positions: _} => {
                        let move_ = Move::by_id(*move_);
                        let other_move_ = Move::by_id(*other_move);
                        if move_.priority_stage == other_move_.priority_stage {
                            let this_spd = pokemon::calculated_stat(state_box, *user_id, StatIndex::Spd);
                            let other_spd = pokemon::calculated_stat(state_box, *other_user_id, StatIndex::Spd);
                            if this_spd == other_spd { rng.gen_bool(0.5) } else { this_spd > other_spd }
                        } else {
                            move_.priority_stage > other_move_.priority_stage
                        }
                    }
                }
            }
        }
    }

    pub fn can_be_performed(&self, state: &mut State, rng: &mut StdRng) -> bool {
        match self {
            Action::Switch {user_id: _, switching_in_id: _} => true,
            Action::Move {user_id, move_: _, move_index, target_positions: _} => {
                let user_msa = state.pokemon_by_id(*user_id).major_status_ailment();
                if user_msa == MajorStatusAilment::Asleep || user_msa == MajorStatusAilment::Frozen || (user_msa == MajorStatusAilment::Paralyzed && rng.gen_bool(0.25)) {
                    if cfg!(feature = "print-battle") {
                        let user_display_text = format!("{}", state.pokemon_by_id(*user_id));
                        state.display_text.push(format!("{}{}", user_display_text, user_msa.display_text_when_blocking_move()));
                    }
                    return false;
                }

                let user = state.pokemon_by_id(*user_id);
                if user.current_hp == 0 || user.field_position == None { return false; }
                match move_index {
                    Some(move_index) => {
                        let move_instance = &user.known_moves[*move_index as usize];
                        move_instance.pp > 0 && !move_instance.disabled
                    },
                    None => true
                }
            }
        }
    }

    /// Called just before can_be_performed() is evaluated.
    pub fn pre_action_stuff(&self, state: &mut State) {
        if let Action::Move {user_id, move_: _, move_index: _, target_positions: _} = self {
            pokemon::increment_msa_counter(state, *user_id);
        }
    }

    pub fn perform(&self, state: &mut State, action_queue: &[&Action], rng: &mut StdRng) -> bool {
        match self {
            Action::Switch {user_id, switching_in_id} => {
                let user_field_pos = state.pokemon_by_id(*user_id).field_position.unwrap();
                pokemon::remove_from_field(state, *user_id);
                pokemon::add_to_field(state, *switching_in_id, user_field_pos)
            },
            Action::Move {user_id, move_: move_id, move_index, target_positions} => {
                let move_ = Move::by_id(*move_id);

                if let Some(move_index) = move_index {
                    state.pokemon_by_id_mut(*user_id).known_moves[*move_index as usize].pp -= 1;
                }

                if cfg!(feature = "print-battle") {
                    let user_display_text = format!("{}", state.pokemon_by_id(*user_id));
                    state.display_text.push(format!("{} used {} on:", user_display_text, move_.name));
                }

                for target_pos in target_positions {
                    let target_id = if *target_pos == FieldPosition::Min {
                        state.min_pokemon_id
                    } else {
                        state.max_pokemon_id
                    };

                    match target_id {
                        Some(target_id) => {
                            if cfg!(feature = "print-battle") {
                                let target_display_text = format!("{}", state.pokemon_by_id(target_id));
                                state.display_text.push(format!("- {}", target_display_text));
                            }

                            let accuracy_check = move_.accuracy == 0 || {
                                let user = state.pokemon_by_id(*user_id);
                                let target = state.pokemon_by_id(target_id);
                                rng.gen_range::<u8, u8, u8>(0, 100) < (move_.accuracy as f64 * accuracy_stat_stage_multiplier(clamp(user.stat_stage(StatIndex::Acc) - target.stat_stage(StatIndex::Eva), -6, 6))) as u8
                            };

                            if accuracy_check {
                                for effect in move_.effects.iter() {
                                    if effect.do_effect(*move_id, state, action_queue, *user_id, target_id, rng) == EffectResult::BattleEnded {
                                        return true;
                                    }
                                }
                            } else if cfg!(feature = "print-battle") {
                                let target_name = Species::name(state.pokemon_by_id(target_id).species);
                                state.display_text.push(format!("{} avoided the attack!", target_name));
                            }
                        },
                        None => {
                            if cfg!(feature = "print-battle") {
                                state.display_text.push(String::from("- None"));
                                state.display_text.push(String::from("But it failed!"));
                            }
                        }
                    }
                }

                false
            }
        }
    }
}

pub type MoveID = u8;

pub struct Move {
    name: String,
    type_: Type,
    category: MoveCategory,
    /// An accuracy of 0 means this move ignores accuracy checks and will always hit.
    accuracy: u8,
    targeting: MoveTargeting,
    max_pp: u8,
    priority_stage: i8,
    sound_based: bool,
    effects: Vec<MoveEffect>
}

impl Move {
    pub fn id_by_name(name: &str) -> Result<MoveID, String> {
        unsafe {
            for (move_id, moves) in MOVES.iter().enumerate() {
                if moves.name.eq_ignore_ascii_case(name) {
                    return Ok(move_id as MoveID);
                }
            }
        }
        Err(format!("Invalid move '{}'", name))
    }

    fn by_id(move_id: MoveID) -> &'static Move {
        unsafe {
            &MOVES[move_id as usize]
        }
    }

    fn category(move_: MoveID) -> MoveCategory {
        let move_ = Move::by_id(move_);
        let category = move_.category;
        if category != MoveCategory::Status && game_version().gen() <= 3 {
            return move_.type_.category();
        }
        category
    }

    pub fn targeting(move_: MoveID) -> MoveTargeting {
        Move::by_id(move_).targeting
    }

    pub fn max_pp(move_: MoveID) -> u8 {
        Move::by_id(move_).max_pp
    }
}

impl Debug for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.debug_struct("Move")
            .field("name", &self.name)
            .field("type_", &self.type_)
            .field("category", &self.category)
            .field("targeting", &self.targeting)
            .field("max_pp", &self.max_pp)
            .field("priority_stage", &self.priority_stage)
            .field("sound_based", &self.sound_based)
            .finish()
    }
}

static mut MOVES: Vec<Move> = vec![];

/// # Safety
/// Should be called after the game version has been set from the program input and before the species are initialized.
pub fn initialize_moves() {
    let mut path = String::from("resources/");
    path.push_str(game_version().name());
    path.push_str("/moves.json");
    let moves_json = fs::read_to_string(path.as_str()).unwrap_or_else(|_| panic!("Failed to read {}.", path));

    match json::parse(moves_json.as_str()) {
        json::Result::Ok(parsed) => {
            match parsed {
                JsonValue::Array(array) => {
                    for member in array {
                        let member_pretty = member.pretty(4);
                        match member {
                            JsonValue::Object(object) => {
                                let extract_string = |key: &str| -> &str {
                                    object.get(key)
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty, key))
                                        .as_str()
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a String", key, member_pretty))
                                };
                                let extract_type = |key: &str| -> Type {
                                    let string = extract_string(key);
                                    Type::by_name(string)
                                        .unwrap_or_else(|_| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid {}", string, member_pretty, key))
                                };
                                let extract_category = |key: &str| -> MoveCategory {
                                    let string = extract_string(key);
                                    MoveCategory::by_name(string)
                                        .unwrap_or_else(|_| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid {}", string, member_pretty, key))
                                };
                                let extract_targeting = |key: &str| -> MoveTargeting {
                                    let string = extract_string(key);
                                    MoveTargeting::by_name(string)
                                        .unwrap_or_else(|_| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid {}", string, member_pretty, key))
                                };
                                let extract_u8 = |key: &str| -> u8 {
                                    object.get(key)
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty, key))
                                        .as_u8()
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid u8 number", key, member_pretty))
                                };
                                let extract_i8 = |key: &str| -> i8 {
                                    object.get(key)
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty, key))
                                        .as_i8()
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid i8 number", key, member_pretty))
                                };
                                let extract_bool = |key: &str| -> bool {
                                    object.get(key)
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty, key))
                                        .as_bool()
                                        .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid boolean", key, member_pretty))
                                };

                                let type_ = extract_type("type");

                                let mut effects = Vec::new();
                                match object.get("effects") {
                                    Some(value) => {
                                        match value {
                                            JsonValue::Array(array2) => {
                                                for member2 in array2 {
                                                    let member_pretty2 = member2.pretty(4);
                                                    match member2 {
                                                        JsonValue::Object(object2) => {
                                                            let extract_string2 = |key: &str| -> &str {
                                                                object2.get(key)
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty2, key))
                                                                    .as_str()
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a String", key, member_pretty2))
                                                            };
                                                            let extract_u82 = |key: &str| -> u8 {
                                                                object2.get(key)
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty2, key))
                                                                    .as_u8()
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid u8 number", key, member_pretty2))
                                                            };
                                                            let extract_u8_def = |key: &str, default: u8| -> u8 {
                                                                object2.get(key)
                                                                    .unwrap_or(&JsonValue::Number(Number::from(default)))
                                                                    .as_u8()
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid u8 number", key, member_pretty2))
                                                            };
                                                            let extract_i82 = |key: &str| -> i8 {
                                                                object2.get(key)
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: object\n{}\ndoes not have a '{}' field", member_pretty2, key))
                                                                    .as_i8()
                                                                    .unwrap_or_else(|| panic!("Invalid moves JSON: '{}' in object\n{}\nis not a valid i8 number", key, member_pretty2))
                                                            };

                                                            let move_effect;
                                                            let name = extract_string2("name");
                                                            match name {
                                                                "StdDamage" => {
                                                                    move_effect = MoveEffect::StdDamage(
                                                                        if member2.has_key("damage_type") {
                                                                            Type::by_name(extract_string2("damage_type"))
                                                                                .unwrap_or_else(|_| panic!("Invalid moves JSON: 'damage_type' in object\n{}\nis not a valid type", member_pretty2))
                                                                        } else { type_ },
                                                                        extract_u82("power"),
                                                                        extract_u8_def("critical_hit_stage_bonus", 0),
                                                                        extract_u8_def("recoil_divisor", 0)
                                                                    );
                                                                },
                                                                "IncTargetStatStage" => {
                                                                    move_effect = MoveEffect::IncTargetStatStage(
                                                                        StatIndex::by_name(extract_string2("stat_index"))
                                                                            .unwrap_or_else(|_| panic!("Invalid moves JSON: 'stat_index' in object\n{}\nis not a valid stat index", member_pretty2)),
                                                                        extract_i82("amount")
                                                                    );
                                                                },
                                                                "LeechSeed" => move_effect = MoveEffect::LeechSeed,
                                                                "Struggle" => move_effect = MoveEffect::Struggle,
                                                                _ => panic!("Invalid moves JSON: '{}' in effect\n{}\nis not a valid move effect", name, member_pretty2)
                                                            }
                                                            effects.push(move_effect);
                                                        },
                                                        _ => panic!("Invalid moves JSON: member\n{}\nin object\n{}\nis not an object", member_pretty2, member_pretty)
                                                    }
                                                }
                                            },
                                            _ => panic!("Invalid moves JSON: 'effects' in object\n{}\nis not an array", member_pretty)
                                        }
                                    },
                                    None => panic!("Invalid moves JSON: object\n{}\ndoes not have an 'effects' field", member_pretty)
                                }

                                let name = extract_string("name");
                                let category = extract_category("category");
                                unsafe {
                                    MOVES.push(Move {
                                        name: name.to_owned(),
                                        type_,
                                        category: if game_version().gen() <= 3 && category != MoveCategory::Status { type_.category() } else { category },
                                        accuracy: extract_u8("accuracy"),
                                        targeting: extract_targeting("targeting"),
                                        max_pp: extract_u8("max_pp"),
                                        priority_stage: extract_i8("priority_stage"),
                                        sound_based: extract_bool("sound_based"),
                                        effects
                                    });
                                }
                            },
                            _ => panic!("Invalid moves JSON: member\n{}\nis not an object", member_pretty)
                        }
                    }
                },
                _ => panic!("Invalid moves JSON: not an array of objects")
            }
        },
        json::Result::Err(error) => {
            println!("{}", error);
            exit(1);
        }
    }
}

enum MoveEffect {
    /// (damage_type: Type, power: u8, critical_hit_stage_bonus: u8, recoil_divisor: u8)
    StdDamage(Type, u8, u8, u8),
    /// (stat_index: StatIndex, amount: i8)
    IncTargetStatStage(StatIndex, i8),
    LeechSeed,
    Struggle
}

impl MoveEffect {
    fn do_effect(&self, move_: MoveID, state: &mut State, action_queue: &[&Action], user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
        match self {
            MoveEffect::StdDamage(damage_type, power, critical_hit_stage_bonus, recoil_divisor) => {
                std_damage(state, user_id, target_id, *damage_type, Move::category(move_), *power, *critical_hit_stage_bonus, *recoil_divisor, rng)
            },
            MoveEffect::IncTargetStatStage(stat_index, amount) => {
                pokemon::increment_stat_stage(state, target_id, *stat_index, *amount);
                EffectResult::Pass
            },
            MoveEffect::LeechSeed => {
                leech_seed(state, user_id, target_id)
            },
            MoveEffect::Struggle => {
                struggle(state, user_id, target_id, rng)
            }
        }
    }
}

#[derive(Eq, PartialEq)]
enum EffectResult {
    Pass,
    Fail,
    BattleEnded
}

fn critical_hit_chance(critical_hit_stage_bonus: u8) -> f64 {
    let mut c = 0;
    c += critical_hit_stage_bonus as usize;
    c = min(c, 4);
    match game_version().gen() {
        1..=5 => [1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 3.0, 1.0 / 2.0][c],
        6 => [1.0 / 16.0, 1.0 / 8.0, 1.0 / 2.0, 1.0, 1.0][c],
        _ => [1.0 / 24.0, 1.0 / 8.0, 1.0 / 2.0, 1.0, 1.0][c]
    }
}

fn main_stat_stage_multiplier(stat_stage: i8) -> f64 {
    max(2, 2 + stat_stage) as f64 / max(2, 2 - stat_stage) as f64
}

fn accuracy_stat_stage_multiplier(stat_stage: i8) -> f64 {
    max(3, 3 + stat_stage) as f64 / max(3, 3 - stat_stage) as f64
}

fn std_base_damage(power: u8, calculated_atk: u32, calculated_def: u32, offensive_stat_stage: i8, defensive_stat_stage: i8, critical_hit: bool) -> u32 {
    let attack_multiplier = if critical_hit && offensive_stat_stage < 0 { 1.0 } else { main_stat_stage_multiplier(offensive_stat_stage) };
    let defense_multiplier = if critical_hit && defensive_stat_stage > 0 { 1.0 } else { main_stat_stage_multiplier(defensive_stat_stage) };
    (42 * power as u32 * (calculated_atk as f64 * attack_multiplier) as u32 / (calculated_def as f64 * defense_multiplier) as u32) / 50 + 2
}

// ---- MOVE EFFECTS ---- //

/// Returns whether the battle has ended.
fn std_damage(state: &mut State, user_id: u8, target_id: u8, damage_type: Type, category: MoveCategory, power: u8, critical_hit_stage_bonus: u8, recoil_divisor: u8, rng: &mut StdRng) -> EffectResult {
    let target_first_type;
    let target_second_type;
    let offensive_stat_index = if category == MoveCategory::Physical { StatIndex::Atk } else { StatIndex::SpAtk };
    let defensive_stat_index = if category == MoveCategory::Physical { StatIndex::Def } else { StatIndex::SpDef };
    let user_ability;
    let user_current_hp;
    let user_max_hp;
    let offensive_stat_stage;
    let defensive_stat_stage;
    let user_major_status_ailment;
    {
        let user = state.pokemon_by_id(user_id);
        let target = state.pokemon_by_id(target_id);
        target_first_type = target.first_type;
        target_second_type = target.second_type;
        user_ability = user.ability;
        user_current_hp = user.current_hp;
        user_max_hp = user.max_hp;
        offensive_stat_stage = user.stat_stage(offensive_stat_index);
        defensive_stat_stage = target.stat_stage(defensive_stat_index);
        user_major_status_ailment = user.major_status_ailment();
    }

    let type_effectiveness = damage_type.effectiveness(target_first_type, target_second_type);
    if almost::zero(type_effectiveness) {
        if cfg!(feature = "print-battle") {
            let target_name = Species::name(state.pokemon_by_id(target_id).species);
            state.display_text.push(format!("It doesn't affect the opponent's {}...", target_name));
        }
        return EffectResult::Fail;
    }

    let mut calculated_atk = pokemon::calculated_stat(state, user_id, offensive_stat_index);
    let calculated_def = pokemon::calculated_stat(state, target_id, defensive_stat_index);

    if damage_type == Type::Grass && user_ability == Ability::id_by_name("Overgrow").unwrap() && user_current_hp < user_max_hp / 3 {
        calculated_atk = (calculated_atk as f64 * 1.5) as u32;
    }

    /*
     Multiply base damage by the following modifiers (in no particular order), rounding up/down at the end
     - Multi-target modifier (TODO?)
     - Weather modifier (TODO)
     - If critical hit, multiply by 1.5 (by 2 prior to 6th gen)
     - Random integer between 85 and 100 divided by 100
     - STAB
     - Type effectiveness
     - Halve damage if user is burned
     - damage = max(damage, 1)
     */

    let mut modified_damage: f64 = if rng.gen_bool(critical_hit_chance(critical_hit_stage_bonus)) {
        if cfg!(feature = "print-battle") {
            state.display_text.push(String::from("It's a critical hit!"));
        }
        std_base_damage(power, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, true) as f64 * if game_version().gen() < 6 { 2.0 } else { 1.5 }
    } else {
        std_base_damage(power, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, false) as f64
    };

    modified_damage *= (100 - rng.gen_range(0, 16)) as f64 / 100.0;
    if damage_type != Type::None && state.pokemon_by_id(user_id).is_type(damage_type) {
        modified_damage *= 1.5;
    }
    modified_damage *= type_effectiveness;
    if cfg!(feature = "print-battle") {
        if type_effectiveness < 0.9 {
            state.display_text.push(String::from("It's not very effective..."));
        } else if type_effectiveness > 1.1 {
            state.display_text.push(String::from("It's super effective!"));
        }
    }
    if user_major_status_ailment == MajorStatusAilment::Burned { modified_damage *= 0.5; }
    modified_damage = modified_damage.max(1.0);

    let damage_dealt = modified_damage.round() as i16;
    if pokemon::apply_damage(state, target_id, damage_dealt) {
        return EffectResult::BattleEnded;
    }
    recoil(state, user_id, damage_dealt, recoil_divisor)
}

fn recoil(state: &mut State, user_id: u8, damage_dealt: i16, recoil_divisor: u8) -> EffectResult {
    if recoil_divisor > 0 {
        let recoil_damage = if game_version().gen() <= 4 {
            max(damage_dealt / recoil_divisor as i16, 1)
        } else {
            max((damage_dealt as f64 / recoil_divisor as f64).round() as i16, 1)
        };
        if cfg!(feature = "print-battle") {
            let user_display_text = format!("{}", state.pokemon_by_id(user_id));
            state.display_text.push(format!("{} took recoil damage!", user_display_text));
        }
        if pokemon::apply_damage(state, user_id, recoil_damage) {
            return EffectResult::BattleEnded;
        }
    }
    EffectResult::Pass
}

/// Returns whether the battle has ended.
fn leech_seed(state: &mut State, user_id: u8, target_id: u8) -> EffectResult {
    match state.pokemon_by_id(target_id).seeded_by {
        Some(_) => {
            if cfg!(feature = "print-battle") {
                let target_name = Species::name(state.pokemon_by_id(target_id).species);
                state.display_text.push(format!("{} is already seeded!", target_name));
            }
            EffectResult::Fail
        },
        None => {
            if state.pokemon_by_id(target_id).is_type(Type::Grass) {
                if cfg!(feature = "print-battle") {
                    let target_name = Species::name(state.pokemon_by_id(target_id).species);
                    state.display_text.push(format!("It doesn't affect the opponent's {}...", target_name));
                }
                EffectResult::Fail
            } else {
                state.pokemon_by_id_mut(target_id).seeded_by = Some(user_id);
                if cfg!(feature = "print-battle") {
                    let target_name = Species::name(state.pokemon_by_id(target_id).species);
                    state.display_text.push(format!("A seed was planted on {}!", target_name));
                }
                EffectResult::Pass
            }
        }
    }
}

/// Returns whether the battle has ended.
fn struggle(state: &mut State, user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
    match game_version().gen() {
        1..=3 => {
            std_damage(state, user_id, target_id, Type::None, MoveCategory::Physical, 50, 0, 4, rng)
        },
        _ => {
            if std_damage(state, user_id, target_id, Type::None, MoveCategory::Physical, 50, 0, 0, rng) == EffectResult::BattleEnded {
                return EffectResult::BattleEnded;
            }
            recoil(state, user_id, state.pokemon_by_id(user_id).max_hp as i16, 4)
        }
    }
}
