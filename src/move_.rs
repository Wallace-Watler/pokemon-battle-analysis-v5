use std::cmp::{max, min};
use std::fmt::{Debug, Error, Formatter};

use rand::Rng;

use crate::{Ability, FieldPosition, game_version, MajorStatusAilment, pokemon, StatIndex, Type, clamp};
use crate::pokemon::Pokemon;
use crate::state::State;
use rand::prelude::StdRng;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
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
    AllPokemon,
}

impl MoveTargeting {
    const fn _single_target(&self) -> bool {
        matches!(self, MoveTargeting::RandomOpponent
                     | MoveTargeting::SingleAdjacentAlly
                     | MoveTargeting::SingleAdjacentOpponent
                     | MoveTargeting::SingleAdjacentPokemon
                     | MoveTargeting::SinglePokemon
                     | MoveTargeting::User
                     | MoveTargeting::UserOrAdjacentAlly)
    }

    const fn _only_targets_allies(&self) -> bool {
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

/// A move selection that will be queued and executed during a turn.
#[derive(Clone, Debug)]
pub struct MoveAction {
    pub user_id: u8,
    pub move_: &'static Move,
    pub move_index: Option<u8>,
    pub target_positions: Vec<FieldPosition>,
}

impl MoveAction {
    /**
     * @param state - a game state
     * @param otherAction - some other move action
     * @return Whether this move action should come before {@code otherAction} based on priority and the user's speed.
     */
    pub fn outspeeds(&self, state_box: &Box<State>, other_action: &MoveAction, rng: &mut StdRng) -> bool {
        if self.move_.priority_stage == other_action.move_.priority_stage {
            let this_spd = pokemon::calculated_stat(state_box, self.user_id, StatIndex::Spd);
            let other_spd = pokemon::calculated_stat(state_box, other_action.user_id, StatIndex::Spd);
            if this_spd == other_spd { rng.gen_bool(0.5) } else { this_spd > other_spd }
        } else {
            self.move_.priority_stage > other_action.move_.priority_stage
        }
    }

    pub fn can_be_performed(&self, state_box: &mut Box<State>, rng: &mut StdRng) -> bool {
        let user_msa = state_box.pokemon[self.user_id as usize].major_status_ailment();
        // TODO: Use seeded RNG
        if user_msa == MajorStatusAilment::Asleep || user_msa == MajorStatusAilment::Frozen || (user_msa == MajorStatusAilment::Paralyzed && rng.gen_bool(0.25)) {
            if cfg!(feature = "print-battle") {
                let user_display_text = format!("{}", &state_box.pokemon[self.user_id as usize]);
                state_box.display_text.push(format!("{}{}", user_display_text, user_msa.display_text_when_blocking_move()));
            }
            return false;
        }

        let user = &state_box.pokemon[self.user_id as usize];
        if user.current_hp == 0 || user.field_position == None { return false; }
        match self.move_index {
            Some(move_index) => {
                let move_instance = &user.known_moves[move_index as usize];
                move_instance.pp > 0 && !move_instance.disabled
            }
            None => true
        }
    }

    /// Called just before can_be_performed is evaluated.
    pub fn pre_move_stuff(&self, state_box: &mut Box<State>) {
        pokemon::increment_msa_counter(state_box, self.user_id);
    }

    pub fn perform(&self, state_box: &mut Box<State>, move_action_queue: &[&MoveAction], rng: &mut StdRng) -> bool {
        if let Some(move_index) = self.move_index {
            state_box.pokemon[self.user_id as usize].known_moves[move_index as usize].pp -= 1;
        }

        if cfg!(feature = "print-battle") {
            let user_display_text = format!("{}", state_box.pokemon[self.user_id as usize]);
            state_box.display_text.push(format!("{} used {} on:", user_display_text, self.move_.name));
        }

        for target_pos in &self.target_positions {
            let target_id = if *target_pos == FieldPosition::Min {
                state_box.min_pokemon_id
            } else {
                state_box.max_pokemon_id
            };

            match target_id {
                Some(target_id) => {
                    if cfg!(feature = "print-battle") {
                        let target_display_text = format!("{}", &state_box.pokemon[target_id as usize]);
                        state_box.display_text.push(format!("- {}", target_display_text));
                    }

                    if (self.move_.effect)(state_box, move_action_queue, self.user_id, target_id, rng) {
                        return true;
                    }
                }
                None => {
                    if cfg!(feature = "print-battle") {
                        state_box.display_text.push(String::from("- None"));
                        state_box.display_text.push(String::from("But it failed!"));
                    }
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
    effect: fn(&mut Box<State>, &[&MoveAction], u8, u8, &mut StdRng) -> bool,
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

impl PartialEq for Move {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Move {}

pub static mut GROWL: Move = Move {
    name: "Growl",
    type_: Type::Normal,
    category: MoveCategory::Status,
    targeting: MoveTargeting::AllAdjacentOpponents,
    max_pp: 40,
    priority_stage: 0,
    sound_based: true,
    effect: growl,
};

pub static mut LEECH_SEED: Move = Move {
    name: "Leech Seed",
    type_: Type::Grass,
    category: MoveCategory::Status,
    targeting: MoveTargeting::SingleAdjacentOpponent,
    max_pp: 10,
    priority_stage: 0,
    sound_based: false,
    effect: leech_seed,
};

pub static mut STRUGGLE: Move = Move {
    name: "Struggle",
    type_: Type::None,
    category: MoveCategory::Physical,
    targeting: MoveTargeting::RandomOpponent,
    max_pp: 1,
    priority_stage: 0,
    sound_based: false,
    effect: struggle,
};

pub static mut TACKLE: Move = Move {
    name: "Tackle",
    type_: Type::Normal,
    category: MoveCategory::Physical,
    targeting: MoveTargeting::SingleAdjacentOpponent,
    max_pp: 35,
    priority_stage: 0,
    sound_based: false,
    effect: tackle,
};

pub static mut VINE_WHIP: Move = Move {
    name: "Vine Whip",
    type_: Type::Grass,
    category: MoveCategory::Physical,
    targeting: MoveTargeting::SingleAdjacentOpponent,
    max_pp: 10,
    priority_stage: 0,
    sound_based: false,
    effect: vine_whip,
};

/// # Safety
/// Should be called after the game version has been set from the program input and before the species are initialized.
pub unsafe fn initialize_moves() {
    VINE_WHIP = Move {
        name: "Vine Whip",
        type_: Type::Grass,
        category: MoveCategory::Physical,
        targeting: MoveTargeting::SingleAdjacentOpponent,
        max_pp: match game_version().gen() {
            1..=3 => 10,
            4..=5 => 15,
            _ => 25
        },
        priority_stage: 0,
        sound_based: false,
        effect: vine_whip,
    };
}

// ---- MOVE FUNCTIONS ---- //

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

fn std_accuracy_check(user: &Pokemon, target: &Pokemon, accuracy: u8, rng: &mut StdRng) -> bool {
    // TODO: Use seeded RNG
    rng.gen_range::<u8, u8, u8>(0, 100) < (accuracy as f64 * accuracy_stat_stage_multiplier(clamp(user.stat_stage(StatIndex::Acc) - target.stat_stage(StatIndex::Eva), -6, 6))) as u8
}

fn std_base_damage(move_power: u32, calculated_atk: u32, calculated_def: u32, offensive_stat_stage: i8, defensive_stat_stage: i8, critical_hit: bool) -> u32 {
    let attack_multiplier = if critical_hit && offensive_stat_stage < 0 { 1.0 } else { main_stat_stage_multiplier(offensive_stat_stage) };
    let defense_multiplier = if critical_hit && defensive_stat_stage > 0 { 1.0 } else { main_stat_stage_multiplier(defensive_stat_stage) };
    (42 * move_power * (calculated_atk as f64 * attack_multiplier) as u32 / (calculated_def as f64 * defense_multiplier) as u32) / 50 + 2
}

fn growl(state_box: &mut Box<State>, _move_queue: &[&MoveAction], user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
    if !std_accuracy_check(&state_box.pokemon[user_id as usize], &state_box.pokemon[target_id as usize], 100, rng) {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("{} avoided the attack!", target_name));
        }
        return false;
    }

    pokemon::increment_stat_stage(state_box, target_id, StatIndex::Atk, -1);
    false
}

fn leech_seed(state_box: &mut Box<State>, _move_queue: &[&MoveAction], user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
    let accuracy_check;
    let target_is_grass_type;
    {
        let user = &state_box.pokemon[user_id as usize];
        let target = &state_box.pokemon[target_id as usize];
        accuracy_check = std_accuracy_check(user, target, 90, rng);
        target_is_grass_type = target.is_type(Type::Grass);
    }

    if !accuracy_check {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("{} avoided the attack!", target_name));
        }
        return false;
    }

    match &state_box.pokemon[target_id as usize].seeded_by {
        Some(_) => {
            if cfg!(feature = "print-battle") {
                let target_name = state_box.pokemon[target_id as usize].species.name;
                state_box.display_text.push(format!("{} is already seeded!", target_name));
            }
        },
        None => {
            if target_is_grass_type {
                if cfg!(feature = "print-battle") {
                    let target_name = state_box.pokemon[target_id as usize].species.name;
                    state_box.display_text.push(format!("It doesn't affect the opponent's {}...", target_name));
                }
            } else {
                state_box.pokemon[target_id as usize].seeded_by = Some(user_id);
                if cfg!(feature = "print-battle") {
                    let target_name = state_box.pokemon[target_id as usize].species.name;
                    state_box.display_text.push(format!("A seed was planted on {}!", target_name));
                }
            }
        }
    }

    false
}

fn struggle(state_box: &mut Box<State>, _move_queue: &[&MoveAction], user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
    let accuracy_check;
    let category = if game_version().gen() <= 3 { Type::Normal.category() } else { MoveCategory::Physical };
    let offensive_stat_index = if category == MoveCategory::Physical { StatIndex::Atk } else { StatIndex::SpAtk };
    let defensive_stat_index = if category == MoveCategory::Physical { StatIndex::Def } else { StatIndex::SpDef };
    let offensive_stat_stage;
    let defensive_stat_stage;
    let user_max_hp;
    let user_major_status_ailment;
    {
        let user = &state_box.pokemon[user_id as usize];
        let target = &state_box.pokemon[target_id as usize];
        accuracy_check = game_version().gen() >= 4 || std_accuracy_check(user, target, 100, rng);
        offensive_stat_stage = user.stat_stage(offensive_stat_index);
        defensive_stat_stage = target.stat_stage(defensive_stat_index);
        user_max_hp = user.max_hp;
        user_major_status_ailment = user.major_status_ailment();
    }

    if !accuracy_check {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("{} avoided the attack!", target_name));
        }
        return false;
    }

    let calculated_atk = pokemon::calculated_stat(state_box, user_id, offensive_stat_index);
    let calculated_def = pokemon::calculated_stat(state_box, target_id, defensive_stat_index);

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

    let mut modified_damage: f64 = if rng.gen_bool(critical_hit_chance(0)) {
        if cfg!(feature = "print-battle") {
            state_box.display_text.push(String::from("It's a critical hit!"));
        }
        std_base_damage(50, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, true) as f64 * if game_version().gen() < 6 { 2.0 } else { 1.5 }
    } else {
        std_base_damage(50, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, false) as f64
    };

    modified_damage *= (100 - rng.gen_range(0, 16)) as f64 / 100.0;
    if user_major_status_ailment == MajorStatusAilment::Burned { modified_damage *= 0.5; }
    modified_damage = modified_damage.max(1.0);

    let damage_dealt = modified_damage.round() as i16;
    if pokemon::apply_damage(state_box, target_id, damage_dealt) {
        return true;
    }

    let recoil_damage = if game_version().gen() <= 3 {
        max(damage_dealt / 4, 1) as i16
    } else if game_version().gen() == 4 {
        max(user_max_hp / 4, 1) as i16
    } else {
        max((user_max_hp as f64 / 4.0).round() as i16, 1)
    };
    if cfg!(feature = "print-battle") {
        let user_display_text = format!("{}", &state_box.pokemon[user_id as usize]);
        state_box.display_text.push(format!("{} took recoil damage!", user_display_text));
    }
    pokemon::apply_damage(state_box, user_id, recoil_damage)
}

fn tackle(state_box: &mut Box<State>, _move_queue: &[&MoveAction], user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
    let accuracy_check;
    let target_first_type;
    let target_second_type;
    let category = if game_version().gen() <= 3 { Type::Normal.category() } else { MoveCategory::Physical };
    let offensive_stat_index = if category == MoveCategory::Physical { StatIndex::Atk } else { StatIndex::SpAtk };
    let defensive_stat_index = if category == MoveCategory::Physical { StatIndex::Def } else { StatIndex::SpDef };
    let offensive_stat_stage;
    let defensive_stat_stage;
    let user_major_status_ailment;
    {
        let user = &state_box.pokemon[user_id as usize];
        let target = &state_box.pokemon[target_id as usize];
        accuracy_check = std_accuracy_check(user, target, if game_version().gen() <= 4 { 95 } else { 100 }, rng);
        target_first_type = target.first_type;
        target_second_type = target.second_type;
        offensive_stat_stage = user.stat_stage(offensive_stat_index);
        defensive_stat_stage = target.stat_stage(defensive_stat_index);
        user_major_status_ailment = user.major_status_ailment();
    }

    if !accuracy_check {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("{} avoided the attack!", target_name));
        }
        return false;
    }

    let damage_type = Type::Normal;
    let type_effectiveness = damage_type.effectiveness(target_first_type, target_second_type);
    if almost::zero(type_effectiveness) {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("It doesn't affect the opponent's {}...", target_name));
        }
        return false;
    }

    let calculated_atk = pokemon::calculated_stat(state_box, user_id, offensive_stat_index);
    let calculated_def = pokemon::calculated_stat(state_box, target_id, defensive_stat_index);

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

    let move_power = match game_version().gen() {
        1..=4 => 35,
        5..=6 => 50,
        _ => 40
    };
    let mut modified_damage: f64 = if rng.gen_bool(critical_hit_chance(0)) {
        if cfg!(feature = "print-battle") {
            state_box.display_text.push(String::from("It's a critical hit!"));
        }
        std_base_damage(move_power, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, true) as f64 * if game_version().gen() < 6 { 2.0 } else { 1.5 }
    } else {
        std_base_damage(move_power, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, false) as f64
    };

    modified_damage *= (100 - rng.gen_range(0, 16)) as f64 / 100.0;
    if damage_type != Type::None && state_box.pokemon[user_id as usize].is_type(damage_type) {
        modified_damage *= 1.5; }
    modified_damage *= type_effectiveness;
    if cfg!(feature = "print-battle") {
        if type_effectiveness < 0.9 {
            state_box.display_text.push(String::from("It's not very effective..."));
        } else if type_effectiveness > 1.1 {
            state_box.display_text.push(String::from("It's super effective!"));
        }
    }
    if user_major_status_ailment == MajorStatusAilment::Burned { modified_damage *= 0.5; }
    modified_damage = modified_damage.max(1.0);

    pokemon::apply_damage(state_box, target_id, modified_damage.round() as i16)
}

fn vine_whip(state_box: &mut Box<State>, _move_queue: &[&MoveAction], user_id: u8, target_id: u8, rng: &mut StdRng) -> bool {
    let accuracy_check;
    let target_first_type;
    let target_second_type;
    let category = if game_version().gen() <= 3 { Type::Grass.category() } else { MoveCategory::Physical };
    let offensive_stat_index = if category == MoveCategory::Physical { StatIndex::Atk } else { StatIndex::SpAtk };
    let defensive_stat_index = if category == MoveCategory::Physical { StatIndex::Def } else { StatIndex::SpDef };
    let offensive_stat_stage;
    let defensive_stat_stage;
    let user_major_status_ailment;
    {
        let user = &state_box.pokemon[user_id as usize];
        let target = &state_box.pokemon[target_id as usize];
        accuracy_check = std_accuracy_check(user, target, 100, rng);
        target_first_type = target.first_type;
        target_second_type = target.second_type;
        offensive_stat_stage = user.stat_stage(offensive_stat_index);
        defensive_stat_stage = target.stat_stage(defensive_stat_index);
        user_major_status_ailment = user.major_status_ailment();
    }

    if !accuracy_check {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("{} avoided the attack!", target_name));
        }
        return false;
    }

    let damage_type = Type::Grass;
    let type_effectiveness = damage_type.effectiveness(target_first_type, target_second_type);
    if almost::zero(type_effectiveness) {
        if cfg!(feature = "print-battle") {
            let target_name = state_box.pokemon[target_id as usize].species.name;
            state_box.display_text.push(format!("It doesn't affect the opponent's {}...", target_name));
        }
        return false;
    }

    let mut calculated_atk = pokemon::calculated_stat(state_box, user_id, offensive_stat_index);
    let calculated_def = pokemon::calculated_stat(state_box, target_id, defensive_stat_index);

    {
        let user = &state_box.pokemon[user_id as usize];
        if user.ability == Ability::Overgrow && user.current_hp < user.max_hp / 3 { calculated_atk = (calculated_atk as f64 * 1.5) as u32; }
    }

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

    let move_power = if game_version().gen() <= 5 { 35 } else { 45 };
    let mut modified_damage: f64 = if rng.gen_bool(critical_hit_chance(0)) {
        if cfg!(feature = "print-battle") {
            state_box.display_text.push(String::from("It's a critical hit!"));
        }
        std_base_damage(move_power, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, true) as f64 * if game_version().gen() < 6 { 2.0 } else { 1.5 }
    } else {
        std_base_damage(move_power, calculated_atk, calculated_def, offensive_stat_stage, defensive_stat_stage, false) as f64
    };

    modified_damage *= (100 - rng.gen_range(0, 16)) as f64 / 100.0;
    if damage_type != Type::None && state_box.pokemon[user_id as usize].is_type(damage_type) {
        modified_damage *= 1.5; }
    modified_damage *= type_effectiveness;
    if cfg!(feature = "print-battle") {
        if type_effectiveness < 0.9 {
            state_box.display_text.push(String::from("It's not very effective..."));
        } else if type_effectiveness > 1.1 {
            state_box.display_text.push(String::from("It's super effective!"));
        }
    }
    if user_major_status_ailment == MajorStatusAilment::Burned { modified_damage *= 0.5; }
    modified_damage = modified_damage.max(1.0);

    pokemon::apply_damage(state_box, target_id, modified_damage.round() as i16)
}
