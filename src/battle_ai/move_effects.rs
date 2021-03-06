use crate::battle_ai::pokemon;
use crate::{FieldPosition, StatIndex, MajorStatusAilment, game_version, Type, Ability, clamp, Weather};
use crate::move_::{MoveCategory, MoveID, Move};
use crate::species::Species;
use crate::battle_ai::state::State;
use rand::prelude::StdRng;
use rand::Rng;
use serde::Deserialize;
use std::cmp::{min, max};
use std::fmt::Debug;

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
    /// No operation. Used whenever only one agent has a choice and the other must do nothing.
    Nop,
    /// An action where the user switches places with a team member not currently on the field.
    Switch {
        user_id: Option<u8>,
        switching_in_id: u8,
        target_position: FieldPosition
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
            Action::Move {user_id: other_user_id, move_: other_move, move_index: _, target_positions: _} => {
                match self {
                    Action::Move {user_id, move_, move_index: _, target_positions: _} => {
                        let priority_stage = Move::priority_stage(*move_);
                        let other_priority_stage = Move::priority_stage(*other_move);
                        if priority_stage == other_priority_stage {
                            let this_spd = pokemon::calculated_stat(state_box, *user_id, StatIndex::Spd);
                            let other_spd = pokemon::calculated_stat(state_box, *other_user_id, StatIndex::Spd);
                            if this_spd == other_spd { rng.gen_bool(0.5) } else { this_spd > other_spd }
                        } else {
                            priority_stage > other_priority_stage
                        }
                    },
                    _ => true
                }
            },
            Action::Nop => false,
            Action::Switch { .. } => matches!(self, Action::Nop)
        }
    }

    pub fn can_be_performed(&self, state: &mut State, rng: &mut StdRng) -> bool {
        match self {
            Action::Move {user_id, move_: _, move_index, target_positions: _} => {
                let user_msa = state.pokemon_by_id(*user_id).major_status_ailment();
                if user_msa == MajorStatusAilment::Asleep || user_msa == MajorStatusAilment::Frozen || (user_msa == MajorStatusAilment::Paralyzed && rng.gen_bool(0.25)) {
                    if cfg!(feature = "print-battle") {
                        let user_display_text = format!("{}", state.pokemon_by_id(*user_id));
                        state.add_display_text(format!("{}{}", user_display_text, user_msa.display_text_when_blocking_move()));
                    }
                    return false;
                }

                let user = state.pokemon_by_id(*user_id);
                if user.current_hp() == 0 || user.field_position() == None { return false; }
                match move_index {
                    Some(move_index) => {
                        let move_instance = user.known_move(*move_index);
                        move_instance.pp > 0 && !move_instance.disabled
                    },
                    None => true
                }
            },
            _ => true
        }
    }

    pub fn perform(&self, state: &mut State, action_queue: &[&Action], rng: &mut StdRng) -> bool {
        match self {
            Action::Switch {user_id, switching_in_id, target_position} => {
                if let Some(user_id) = user_id {
                    pokemon::remove_from_field(state, *user_id);
                }
                pokemon::add_to_field(state, *switching_in_id, *target_position)
            },
            Action::Move {user_id, move_: move_id, move_index, target_positions} => {
                if let Some(move_index) = move_index {
                    pokemon::increment_move_pp(state, *user_id, *move_index, -1);
                }

                if cfg!(feature = "print-battle") {
                    let user_display_text = format!("{}", state.pokemon_by_id(*user_id));
                    state.add_display_text(format!("{} used {} on:", user_display_text, Move::name(*move_id)));
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
                                state.add_display_text(format!("- {}", target_display_text));
                            }

                            if Move::accuracy(*move_id).do_accuracy_check(state, *user_id, target_id, rng) {
                                for effect in Move::effects(*move_id) {
                                    if effect.do_effect(*move_id, state, action_queue, *user_id, target_id, rng) == EffectResult::BattleEnded {
                                        return true;
                                    }
                                }
                            } else if cfg!(feature = "print-battle") {
                                let target_name = Species::name(state.pokemon_by_id(target_id).species());
                                state.add_display_text(format!("{} avoided the attack!", target_name));
                            }
                        },
                        None => {
                            if cfg!(feature = "print-battle") {
                                state.add_display_text(String::from("- None"));
                                state.add_display_text(String::from("But it failed!"));
                            }
                        }
                    }
                }

                false
            },
            Action::Nop => false
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum MoveEffect {
    Growth,
    /// (stat_index: StatIndex, amount: i8)
    IncTargetStatStage(StatIndex, i8),
    LeechSeed,
    PoisonPowder,
    SleepPowder,
    /// (damage_type: Type, power: u8, critical_hit_stage_bonus: u8, recoil_divisor: u8)
    StdDamage(Type, u8, u8, u8),
    Struggle,
    Synthesis,
    Toxic
}

impl MoveEffect {
    fn do_effect(&self, move_: MoveID, state: &mut State, action_queue: &[&Action], user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
        match self {
            MoveEffect::Growth => growth(state, user_id),
            MoveEffect::IncTargetStatStage(stat_index, amount) => {
                pokemon::increment_stat_stage(state, target_id, *stat_index, *amount);
                EffectResult::Success
            },
            MoveEffect::LeechSeed => leech_seed(state, user_id, target_id),
            MoveEffect::PoisonPowder => poison_powder(state, target_id),
            MoveEffect::SleepPowder => sleep_powder(state, target_id),
            MoveEffect::StdDamage(damage_type, power, critical_hit_stage_bonus, recoil_divisor) => {
                std_damage(state, user_id, target_id, *damage_type, Move::category(move_), *power, *critical_hit_stage_bonus, *recoil_divisor, rng)
            },
            MoveEffect::Struggle => struggle(state, user_id, target_id, rng),
            MoveEffect::Synthesis => synthesis(state, user_id),
            MoveEffect::Toxic => toxic(state, target_id)
        }
    }
}

/// The possible outcomes that a move's effect can lead to.
#[derive(Eq, PartialEq)]
enum EffectResult {
    Success,
    Fail,
    BattleEnded
}

#[derive(Debug, Deserialize)]
pub enum MoveAccuracy {
    Ignore,
    /// (percentage: u8)
    Standard(u8),
    Toxic
}

impl MoveAccuracy {
    fn do_accuracy_check(&self, state: &mut State, user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
        match self {
            MoveAccuracy::Ignore => true,
            MoveAccuracy::Standard(accuracy) => MoveAccuracy::std_accuracy_check(state, *accuracy, user_id, target_id, rng),
            MoveAccuracy::Toxic => {
                (game_version().gen() >= 6 && state.pokemon_by_id(user_id).is_type(Type::Poison))
                    || MoveAccuracy::std_accuracy_check(state, if game_version().gen() <= 4 { 85 } else { 90 }, user_id, target_id, rng)
            }
        }
    }

    fn std_accuracy_check(state: &mut State, accuracy: u8, user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
        let user = state.pokemon_by_id(user_id);
        let target = state.pokemon_by_id(target_id);
        rng.gen_range::<u8, u8, u8>(0, 100) < (accuracy as f64 * accuracy_stat_stage_multiplier(clamp(user.stat_stage(StatIndex::Acc) - target.stat_stage(StatIndex::Eva), -6, 6))) as u8
    }
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
        target_first_type = target.first_type();
        target_second_type = target.second_type();
        user_ability = user.ability();
        user_current_hp = user.current_hp();
        user_max_hp = user.max_hp();
        offensive_stat_stage = user.stat_stage(offensive_stat_index);
        defensive_stat_stage = target.stat_stage(defensive_stat_index);
        user_major_status_ailment = user.major_status_ailment();
    }

    let type_effectiveness = damage_type.effectiveness(target_first_type, target_second_type);
    if almost::zero(type_effectiveness) {
        if cfg!(feature = "print-battle") {
            let target_name = Species::name(state.pokemon_by_id(target_id).species());
            state.add_display_text(format!("It doesn't affect the opponent's {}...", target_name));
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
            state.add_display_text(String::from("It's a critical hit!"));
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
            state.add_display_text(String::from("It's not very effective..."));
        } else if type_effectiveness > 1.1 {
            state.add_display_text(String::from("It's super effective!"));
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
            state.add_display_text(format!("{} took recoil damage!", user_display_text));
        }
        if pokemon::apply_damage(state, user_id, recoil_damage) {
            return EffectResult::BattleEnded;
        }
    }
    EffectResult::Success
}

fn growth(state: &mut State, user_id: u8) -> EffectResult {
    if game_version().gen() <= 4 {
        pokemon::increment_stat_stage(state, user_id, StatIndex::SpAtk, 1);
    } else {
        let requested_amount = if state.weather == Weather::HarshSunshine { 2 } else { 1 };
        pokemon::increment_stat_stage(state, user_id, StatIndex::Atk, requested_amount);
        pokemon::increment_stat_stage(state, user_id, StatIndex::SpAtk, requested_amount);
    }
    EffectResult::Success
}

fn leech_seed(state: &mut State, user_id: u8, target_id: u8) -> EffectResult {
    match state.pokemon_by_id(target_id).seeded_by {
        Some(_) => {
            if cfg!(feature = "print-battle") {
                let target_name = Species::name(state.pokemon_by_id(target_id).species());
                state.add_display_text(format!("{} is already seeded!", target_name));
            }
            EffectResult::Fail
        },
        None => {
            if state.pokemon_by_id(target_id).is_type(Type::Grass) {
                if cfg!(feature = "print-battle") {
                    let target_name = Species::name(state.pokemon_by_id(target_id).species());
                    state.add_display_text(format!("It doesn't affect the opponent's {}...", target_name));
                }
                EffectResult::Fail
            } else {
                state.pokemon_by_id_mut(target_id).seeded_by = state.pokemon_by_id(user_id).field_position();
                if cfg!(feature = "print-battle") {
                    let target_name = Species::name(state.pokemon_by_id(target_id).species());
                    state.add_display_text(format!("A seed was planted on {}!", target_name));
                }
                EffectResult::Success
            }
        }
    }
}

fn poison_powder(state: &mut State, target_id: u8) -> EffectResult {
    if game_version().gen() >= 6 && state.pokemon_by_id(target_id).is_type(Type::Grass) {
        if cfg!(feature = "print-battle") {
            let species_name = Species::name(state.pokemon_by_id(target_id).species());
            state.add_display_text(format!("It doesn't affect the opponent's {} ...", species_name));
        }
        return EffectResult::Fail;
    }
    if pokemon::poison(state, target_id, false) {
        EffectResult::Success
    } else {
        EffectResult::Fail
    }
}

fn sleep_powder(state: &mut State, target_id: u8) -> EffectResult {
    if game_version().gen() >= 6 && state.pokemon_by_id(target_id).is_type(Type::Grass) {
        if cfg!(feature = "print-battle") {
            let species_name = Species::name(state.pokemon_by_id(target_id).species());
            state.add_display_text(format!("It doesn't affect the opponent's {} ...", species_name));
        }
        return EffectResult::Fail;
    }
    if pokemon::put_to_sleep(state, target_id) {
        EffectResult::Success
    } else {
        EffectResult::Fail
    }
}

fn struggle(state: &mut State, user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
    match game_version().gen() {
        1..=3 => {
            std_damage(state, user_id, target_id, Type::None, MoveCategory::Physical, 50, 0, 4, rng)
        },
        _ => {
            if std_damage(state, user_id, target_id, Type::None, MoveCategory::Physical, 50, 0, 0, rng) == EffectResult::BattleEnded {
                return EffectResult::BattleEnded;
            }
            recoil(state, user_id, state.pokemon_by_id(user_id).max_hp() as i16, 4)
        }
    }
}

fn synthesis(state: &mut State, user_id: u8) -> EffectResult {
    let mut max_hp = state.pokemon_by_id(user_id).current_hp() as i16;
    match state.weather {
        Weather::None | Weather::StrongWinds => max_hp /= 2,
        Weather::HarshSunshine => max_hp = max_hp * 2 / 3,
        _ => max_hp /= 4
    }
    pokemon::apply_damage(state, user_id, -max_hp);
    EffectResult::Success
}

fn toxic(state: &mut State, target_id: u8) -> EffectResult {
    if pokemon::badly_poison(state, target_id, false) {
        EffectResult::Success
    } else {
        EffectResult::Fail
    }
}
