use crate::battle_ai::pokemon;
use rand::prelude::StdRng;
use rand::Rng;
use serde::Deserialize;
use std::cmp::{min, max, Ordering};
use std::fmt::Debug;
use crate::battle_ai::data::{StatIndex, Type, Weather, MajorStatusAilment, Gender};
use crate::battle_ai::move_::{MoveID, Move, MoveCategory};
use crate::battle_ai::state::{State, Action, Counter};
use crate::battle_ai::species::Species;

#[derive(Debug, Deserialize)]
pub enum MoveEffect {
    Attract,
    GigaDrain,
    Growth,
    /// (stat_index: StatIndex, amount: i8)
    IncTargetStatStage(StatIndex, i8),
    LeechSeed,
    /// (toxic: bool, chance: u8)
    Poison(bool, u8),
    PoisonPowder,
    SleepPowder,
    /// (damage_type: Type, power: u8, critical_hit_stage_bonus: u8)
    StdDamage(Type, u8, u8),
    Struggle,
    SunnyDay,
    Synthesis
}

impl MoveEffect {
    fn do_effect(&self, move_: MoveID, state: &mut State, action_queue: &[&Action], user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
        match self {
            MoveEffect::Attract => attract(state, user_id, target_id),
            MoveEffect::GigaDrain => giga_drain(state, user_id, target_id, rng),
            MoveEffect::Growth => growth(state, user_id),
            MoveEffect::IncTargetStatStage(stat_index, amount) => {
                pokemon::increment_stat_stage(state, target_id, *stat_index, *amount);
                EffectResult::Success
            },
            MoveEffect::LeechSeed => leech_seed(state, user_id, target_id),
            MoveEffect::Poison(toxic, chance) => {
                if rng.gen_range(0, 100) < *chance {
                    pokemon::poison(state, target_id, *toxic, false)
                } else {
                    EffectResult::Skip
                }
            },
            MoveEffect::PoisonPowder => poison_powder(state, target_id),
            MoveEffect::SleepPowder => sleep_powder(state, target_id, rng),
            MoveEffect::StdDamage(damage_type, power, critical_hit_stage_bonus) => {
                std_damage(state, user_id, target_id, *damage_type, Move::category(move_), *power, *critical_hit_stage_bonus, rng).0
            },
            MoveEffect::Struggle => struggle(state, user_id, target_id, rng),
            MoveEffect::SunnyDay => sunny_day(state),
            MoveEffect::Synthesis => synthesis(state, user_id)
        }
    }
}

/// The possible outcomes that a move's effect can lead to.
#[derive(Eq, PartialEq)]
pub enum EffectResult {
    /// It tried and failed to do the effect
    Fail,
    /// It didn't have any effect
    NoEffect,
    /// The effect was skipped
    Skip,
    /// It tried and succeeded in doing the effect
    Success
}

impl EffectResult {
    const fn has_display_text(&self) -> bool {
        matches!(self, EffectResult::Fail | EffectResult::NoEffect)
    }

    const fn display_text(&self) -> &'static str {
        match self {
            EffectResult::Fail => "But it failed!",
            EffectResult::NoEffect => "It didn't have any effect...",
            _ => ""
        }
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

fn std_damage(state: &mut State, user_id: u8, target_id: u8, damage_type: Type, category: MoveCategory, power: u8, critical_hit_stage_bonus: u8, rng: &mut StdRng) -> (EffectResult, u16) {
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
        return (EffectResult::NoEffect, 0);
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

    if state.weather == Weather::HarshSunshine {
        modified_damage *= match damage_type {
            Type::Fire => 1.5,
            Type::Water => 0.5,
            _ => 1.0
        };
    }

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

    let damage_dealt = modified_damage.round() as u16;
    pokemon::apply_damage(state, target_id, damage_dealt as i16);
    (EffectResult::Success, damage_dealt)
}

fn recoil(state: &mut State, user_id: u8, numerator: u16, denominator: u8) -> EffectResult {
    if cfg!(feature = "print-battle") {
        let user_name = Species::name(state.pokemon_by_id(user_id).species());
        state.add_display_text(format!("{} took recoil damage!", user_name));
    }

    let recoil_damage = if game_version().gen() <= 4 {
        max(numerator / denominator as u16, 1)
    } else {
        max((numerator as f64 / denominator as f64).round() as u16, 1)
    };
    pokemon::apply_damage(state, user_id, recoil_damage as i16);
    EffectResult::Success
}

fn attract(state: &mut State, user_id: u8, target_id: u8) -> EffectResult {
    let user_gender = state.pokemon_by_id(user_id).gender;
    let target_gender = state.pokemon_by_id(target_id).gender;
    if user_gender == target_gender.opposite() && target_gender != Gender::None {
        pokemon::set_infatuated(state, target_id, user_id);
        EffectResult::Success
    } else {
        EffectResult::Fail
    }
}

fn giga_drain(state: &mut State, user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
    let (result, damage_dealt) = std_damage(state, user_id, target_id, Type::Grass, MoveCategory::Special, if game_version().gen() <= 4 { 60 } else { 75 }, 0, rng);

    if result == EffectResult::Success && !state.has_battle_ended() {
        if cfg!(feature = "print-battle") {
            let target_name = Species::name(state.pokemon_by_id(target_id).species());
            state.add_display_text(format!("{} had its health drained!", target_name));
        }
        pokemon::apply_damage(state, user_id, -max(damage_dealt as i16 / 2, 1));
    }

    result
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
        Some(_) => EffectResult::Fail,
        None => {
            if state.pokemon_by_id(target_id).is_type(Type::Grass) {
                EffectResult::NoEffect
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
        return EffectResult::NoEffect;
    }
    pokemon::poison(state, target_id, false, false)
}

fn sleep_powder(state: &mut State, target_id: u8, rng: &mut StdRng) -> EffectResult {
    if game_version().gen() >= 6 && state.pokemon_by_id(target_id).is_type(Type::Grass) {
        return EffectResult::NoEffect;
    }
    pokemon::put_to_sleep(state, target_id, rng)
}

fn struggle(state: &mut State, user_id: u8, target_id: u8, rng: &mut StdRng) -> EffectResult {
    let (result, damage_dealt) = std_damage(state, user_id, target_id, Type::None, MoveCategory::Physical, 50, 0, rng);
    if result != EffectResult::Success {
        return result;
    }

    match game_version().gen() {
        1..=3 => recoil(state, user_id, damage_dealt, 4),
        _ => recoil(state, user_id, state.pokemon_by_id(user_id).max_hp(), 4)
    }
}

fn sunny_day(state: &mut State) -> EffectResult {
    if (game_version().gen() >= 3 && state.weather == Weather::HarshSunshine) || (game_version().gen() >= 5 && matches!(state.weather, Weather::HeavyRain | Weather::ExtremelyHarshSunshine | Weather::StrongWinds)) {
        return EffectResult::Fail;
    }

    state.weather = Weather::HarshSunshine;
    state.weather_counter = Counter::new(Some(5));
    if cfg!(feature = "print-battle") {
        state.add_display_text(Weather::HarshSunshine.display_text_on_appearance().to_owned());
    }
    EffectResult::Success
}

fn synthesis(state: &mut State, user_id: u8) -> EffectResult {
    if cfg!(feature = "print-battle") {
        let species_name = Species::name(state.pokemon_by_id(user_id).species());
        state.add_display_text(format!("{} restored its HP!", species_name));
    }

    let mut max_hp = state.pokemon_by_id(user_id).current_hp() as i16;
    match state.weather {
        Weather::None | Weather::StrongWinds => max_hp /= 2,
        Weather::HarshSunshine => max_hp = max_hp * 2 / 3,
        _ => max_hp /= 4
    }
    pokemon::apply_damage(state, user_id, -max_hp);
    EffectResult::Success
}
