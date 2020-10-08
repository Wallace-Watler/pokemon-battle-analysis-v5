use crate::{FieldPosition, StatIndex, pokemon, MajorStatusAilment, Type, game_version, clamp};
use crate::state::StateSpace;
use rand::Rng;
use std::fmt::{Debug, Formatter, Error};
use std::cmp::{min, max};
use crate::pokemon::Pokemon;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MoveCategory {
    Physical,
    Special,
    Status
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
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
    const fn single_target(&self) -> bool {
        match self {
            MoveTargeting::RandomOpponent | MoveTargeting::SingleAdjacentAlly | MoveTargeting::SingleAdjacentOpponent | MoveTargeting::SingleAdjacentPokemon | MoveTargeting::SinglePokemon | MoveTargeting::User | MoveTargeting::UserOrAdjacentAlly => true,
            _ => false
        }
    }

    const fn only_targets_allies(&self) -> bool {
        match self {
            MoveTargeting::SingleAdjacentAlly | MoveTargeting::User | MoveTargeting::UserOrAdjacentAlly | MoveTargeting::UserAndAllAllies | MoveTargeting::AllAllies => true,
            _ => false
        }
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

/// A move selection that will be queued and executed during a turn.
#[derive(Clone, Debug)]
pub struct MoveAction {
    pub user_id: u8,
    pub move_: &'static Move,
    pub move_index: Option<usize>,
    pub target_positions: Vec<FieldPosition>
}

impl MoveAction {
    /**
     * @param state - a game state
     * @param otherAction - some other move action
     * @return Whether this move action should come before {@code otherAction} based on priority and the user's speed.
     */
    pub fn outspeeds(&self, state_space: &StateSpace, state_id: usize, other_action: &MoveAction) -> bool {
        // TODO: Use seeded RNG
        if self.move_.priority_stage == other_action.move_.priority_stage {
            let this_spd = pokemon::calculated_stat(state_space, state_id, self.user_id, StatIndex::Spd);
            let other_spd = pokemon::calculated_stat(state_space, state_id, other_action.user_id, StatIndex::Spd);
            if this_spd == other_spd { rand::thread_rng().gen_bool(0.5) } else { this_spd > other_spd }
        } else {
            self.move_.priority_stage > other_action.move_.priority_stage
        }
    }

    pub fn can_be_performed(&self, state_space: &mut StateSpace, state_id: usize) -> bool {
        let state = state_space.get_mut(state_id).unwrap();
        let user_major_status_ailment;
        let user_display_text;
        {
            let user = state.pokemon_by_id(self.user_id);
            user_major_status_ailment = user.major_status_ailment();
            user_display_text = format!("{}", user);
        }

        if user_major_status_ailment == MajorStatusAilment::Asleep {
            state.display_text.push(format!("{} is fast asleep.", user_display_text));
            return false;
        }
        if user_major_status_ailment == MajorStatusAilment::Frozen {
            state.display_text.push(format!("{} is frozen solid!", user_display_text));
            return false;
        }
        // TODO: Use seeded RNG
        if user_major_status_ailment == MajorStatusAilment::Paralyzed && rand::thread_rng().gen_bool(0.25) {
            state.display_text.push(format!("{} is paralyzed! It can't move!", user_display_text));
            return false;
        }

        let user = state.pokemon_by_id(self.user_id);
        if user.current_hp == 0 || user.field_position == None { return false; }
        match self.move_index {
            Some(move_index) => {
                let move_instance = user.known_moves.get(move_index).unwrap();
                move_instance.pp > 0 && !move_instance.disabled
            },
            None => true
        }
    }

    /// Called just before can_be_performed is evaluated.
    pub fn pre_move_stuff(&self, state_space: &mut StateSpace, state_id: usize) {
        pokemon::increment_msa_counter(state_space, state_id, self.user_id);
    }

    pub fn perform(&self, state_space: &mut StateSpace, state_id: usize, move_action_queue: &[&MoveAction]) -> bool {
        if let Some(move_index) = self.move_index {
            state_space.get_mut(state_id).unwrap().pokemon_by_id_mut(self.user_id).known_moves.get_mut(move_index).unwrap().pp -= 1;
        }

        let user_display_text = format!("{}", state_space.get_mut(state_id).unwrap().pokemon_by_id_mut(self.user_id));
        state_space.get_mut(state_id).unwrap().display_text.push(format!("{} used {} on:", user_display_text, self.move_.name));

        for target_pos in &self.target_positions {
            let target_id = if *target_pos == FieldPosition::Min {
                state_space.get(state_id).unwrap().min_pokemon_id
            } else {
                state_space.get(state_id).unwrap().max_pokemon_id
            };

            match target_id {
                Some(target_id) => {
                    let target_display_text = format!("{}", state_space.get(state_id).unwrap().pokemon_by_id(target_id));
                    state_space.get_mut(state_id).unwrap().display_text.push(format!("- {}", target_display_text));
                    if (self.move_.effect)(state_space, state_id, move_action_queue, self.user_id, target_id) {
                        return true;
                    }
                },
                None => {
                    state_space.get_mut(state_id).unwrap().display_text.push(String::from("- None"));
                    state_space.get_mut(state_id).unwrap().display_text.push(String::from("But it failed!"));
                }
            }
        }

        false
    }
}

/*
#[derive(Clone, Debug, Hash)]
pub struct SwitchAction {
    user_id: u8,
    switching_in_id: u8
}
*/

pub struct Move {
    name: &'static str,
    type_: Type,
    category: MoveCategory,
    pub targeting: MoveTargeting,
    pub max_pp: u8,
    priority_stage: i8,
    sound_based: bool,
    effect: fn(&mut StateSpace, usize, &[&MoveAction], u8, u8) -> bool
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

pub static mut STRUGGLE: Move = Move {
    name: "Struggle",
    type_: Type::None,
    category: MoveCategory::Physical,
    targeting: MoveTargeting::RandomOpponent,
    max_pp: 1,
    priority_stage: 0,
    sound_based: false,
    effect: struggle
};

// ---- MOVE FUNCTIONS ---- //

/// Allows easy function chaining.
macro_rules! compose {
    ($last:expr) => { $last };
    ($head:expr, $($tail:expr), +) => { |x| compose!($($tail), +)($head(x)) }
}

fn critical_hit_chance(critical_hit_stage_bonus: usize) -> f64 {
    let mut c: usize = 0;
    c += critical_hit_stage_bonus;
    c = min(c, 4);
    if game_version().gen() <= 5 {
        [1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 3.0, 1.0 / 2.0][c]
    } else if game_version().gen() == 6 {
        [1.0 / 16.0, 1.0 / 8.0, 1.0 / 2.0, 1.0, 1.0][c]
    } else {
        [1.0 / 24.0, 1.0 / 8.0, 1.0 / 2.0, 1.0, 1.0][c]
    }
}

fn main_stat_stage_multiplier(stat_stage: i8) -> f64 {
    max(2, 2 + stat_stage) as f64 / max(2, 2 - stat_stage) as f64
}

fn accuracy_stat_stage_multiplier(stat_stage: i8) -> f64 {
    max(3, 3 + stat_stage) as f64 / max(3, 3 - stat_stage) as f64
}

fn std_accuracy_check(user: &Pokemon, target: &Pokemon, accuracy: u8) -> bool {
    // TODO: Use seeded RNG
    rand::thread_rng().gen_range::<u8, u8, u8>(0, 100) < (accuracy as f64 * accuracy_stat_stage_multiplier(clamp(user.stat_stage(StatIndex::Acc) - target.stat_stage(StatIndex::Eva), -6, 6))) as u8
}

fn std_base_damage(move_power: u32, calculated_atk: u32, calculated_def: u32, offensive_stat_stage: i8, defensive_stat_stage: i8, critical_hit: bool) -> u32 {
    let attack_multiplier = if critical_hit && offensive_stat_stage < 0 { 1.0 } else { main_stat_stage_multiplier(offensive_stat_stage) };
    let defense_multiplier = if critical_hit && defensive_stat_stage > 0 { 1.0 } else { main_stat_stage_multiplier(defensive_stat_stage) };
    (42 * move_power * (calculated_atk as f64 * attack_multiplier) as u32 / (calculated_def as f64 * defense_multiplier) as u32) / 50 + 2
}

fn struggle(state_space: &mut StateSpace, state_id: usize, move_queue: &[&MoveAction], user_id: u8, target_id: u8) -> bool {
    let accuracy_check;
    let target_name;
    let category = if game_version().gen() <= 3 { Type::Normal.category() } else { MoveCategory::Physical };
    let offensive_stat_index = if category == MoveCategory::Physical { StatIndex::Atk } else { StatIndex::SpAtk };
    let defensive_stat_index = if category == MoveCategory::Physical { StatIndex::Def } else { StatIndex::SpDef };
    let offensive_stat_stage;
    let defensive_stat_stage;
    let user_max_hp;
    let user_major_status_ailment;
    {
        let state = state_space.get(state_id).unwrap();
        let user = state.pokemon_by_id(user_id);
        let target = state.pokemon_by_id(target_id);
        accuracy_check = game_version().gen() >= 4 || std_accuracy_check(user, target, 100);
        target_name = target.species.name;
        offensive_stat_stage = user.stat_stage(offensive_stat_index);
        defensive_stat_stage = target.stat_stage(defensive_stat_index);
        user_max_hp = user.max_hp;
        user_major_status_ailment = user.major_status_ailment();
    }
    {
        let state_mut = state_space.get_mut(state_id).unwrap();
        if !accuracy_check {
            state_mut.display_text.push(format!("{} avoided the attack!", target_name));
            return false;
        }
    }

    let calculated_atk = pokemon::calculated_stat(state_space, state_id, user_id, offensive_stat_index);
    let calculated_def = pokemon::calculated_stat(state_space, state_id, target_id, defensive_stat_index);

    /*
     Multiply base damage by the following modifiers (in no particular order), rounding up/down at the end
     - Multi-target modifier (?)
     - Weather modifier (TODO)
     - If critical hit, multiply by 1.5 (by 2 prior to 6th gen)
     - Random integer between 85 and 100 divided by 100
     - STAB
     - Type effectiveness
     - Halve damage if user is burned
     - damage = max(damage, 1)
     */

    // TODO: Use seeded RNG
    let mut modified_damage: f64 = if rand::thread_rng().gen_bool(critical_hit_chance(0)) {
        state_space.get_mut(state_id).unwrap().display_text.push(String::from("It's a critical hit!"));
        std_base_damage(50, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, true) as f64 * if game_version().gen() < 6 { 2.0 } else { 1.5 }
    } else {
        std_base_damage(50, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, false) as f64
    };

    modified_damage *= (100 - rand::thread_rng().gen_range(0, 16)) as f64 / 100.0;
    if user_major_status_ailment == MajorStatusAilment::Burned { modified_damage *= 0.5; }
    modified_damage = modified_damage.max(1.0);

    let damage_dealt = modified_damage.round() as i16;
    if pokemon::apply_damage(state_space, state_id, target_id, damage_dealt) {
        return true;
    }

    let recoil_damage = if game_version().gen() <= 3 {
        max(damage_dealt / 4, 1) as i16
    } else if game_version().gen() == 4 {
        max(user_max_hp / 4, 1) as i16
    } else {
        max((user_max_hp as f64 / 4.0).round() as i16, 1)
    };
    let user_display_text = format!("{}", state_space.get(state_id).unwrap().pokemon_by_id(user_id));
    state_space.get_mut(state_id).unwrap().display_text.push(format!("{} took recoil damage!", user_display_text));
    pokemon::apply_damage(state_space, state_id, user_id, recoil_damage)
}
