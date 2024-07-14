use std::cmp::{max, min, Ordering};

use rand::prelude::StdRng;
use rand::Rng;

use crate::battle_ai::game_theory::{Matrix, ZeroSumNashEq};
use crate::battle_ai::move_effects::Action;
use crate::move_::{Move, MoveCategory};
use std::ops::AddAssign;
use num::{One, Zero};
use crate::battle_ai::data::{Weather, Terrain, FieldPosition, Type, Gender, Nature, MajorStatusAilment, StatIndex};
use crate::battle_ai::species::{SpeciesID, Species};
use crate::battle_ai::move_::{MoveID, Move};

/// How many turns ahead the agents compute
pub const AI_LEVEL: u8 = 3;

/// Maximum number of times each agent is allowed to switch out Pokemon before it must choose a move
/// (does not count switching one in to replace a fainted team member)
const CONSECUTIVE_SWITCH_CAP: u16 = 1;

pub static mut NUM_STATE_COPIES: u64 = 0;

/// Represents the entire game state of a battle.
#[derive(Clone, Debug)]
pub struct State {
    /// ID is the index; IDs 0-5 is the minimizing team, 6-11 is the maximizing team.
    pub pokemon: [Pokemon; 12],
    pub max: Agent,
    pub min: Agent,
    pub weather: Weather,
    pub weather_counter: Counter<u16>,
    pub terrain: Terrain,
    turn_number: u16,
    /// Battle print-out that is shown when this state is entered; useful for sanity checks.
    display_text: Vec<String>,
    children: Vec<Option<Box<State>>>,
}

impl State {
    fn new(pokemon: [Pokemon; 12], weather: Weather, terrain: Terrain) -> State {
        State {
            pokemon,
            max: Agent {
                on_field: None,
                actions: vec![
                    Action::Switch {
                        user_id: None,
                        switching_in_id: 6,
                        target_position: FieldPosition::Max
                    }
                ],
                action_order: vec![0],
                consecutive_switches: 0
            },
            min: Agent {
                on_field: None,
                actions: vec![
                    Action::Switch {
                        user_id: None,
                        switching_in_id: 0,
                        target_position: FieldPosition::Min
                    }
                ],
                action_order: vec![0],
                consecutive_switches: 0
            },
            weather,
            weather_counter: Counter::new(None),
            terrain,
            turn_number: 0,
            display_text: Vec::new(),
            children: vec![None; 1]
        }
    }

    pub const fn pokemon_by_id(&self, pokemon_id: u8) -> &Pokemon {
        &self.pokemon[pokemon_id as usize]
    }

    pub fn pokemon_by_id_mut(&mut self, pokemon_id: u8) -> &mut Pokemon {
        &mut self.pokemon[pokemon_id as usize]
    }

    pub fn add_display_text(&mut self, text: String) {
        self.display_text.push(text);
    }

    fn print_display_text(&self) {
        self.display_text.iter().for_each(|text| {
            text.lines().for_each(|line| println!("  {}", line));
        });
    }

    pub fn has_battle_ended(&self) -> bool {
        self.pokemon[0..6].iter().all(|pokemon| pokemon.current_hp() == 0) || self.pokemon[6..12].iter().all(|pokemon| pokemon.current_hp() == 0)
    }
}

#[derive(Clone, Debug)]
pub struct Agent {
    /// Pokemon owned by this agent that is on the field
    pub on_field: Option<u8>,
    actions: Vec<Action>,
    action_order: Vec<usize>,
    consecutive_switches: u16
}

// TODO: Store static info outside of Pokemon
#[derive(Clone, Debug)]
/// Assumed to be level 100.
pub struct Pokemon {
    pub species: SpeciesID,
    // Types usually match the species' type, but some Pokemon can change types
    first_type: Type,
    second_type: Type,
    pub gender: Gender,
    nature: Nature,
    ability: AbilityID,
    ivs: [u8; 6],
    evs: [u8; 6],
    max_hp: u16,
    current_hp: u16,
    stat_stages: [i8; 8],

    // Major status ailment
    major_status_ailment: MajorStatusAilment,
    pub msa_counter: Counter<u16>,
    /// Only used in gen 3; otherwise it's always 0.
    snore_sleep_talk_counter: u16,

    // Minor status ailments
    confusion_counter: Counter<u16>,
    is_flinching: bool,
    /// Position of the Pokemon that seeded this Pokemon.
    pub seeded_by: Option<FieldPosition>,
    pub is_infatuated: bool,
    is_cursed: bool,
    has_nightmare: bool,

    field_position: Option<FieldPosition>,
    known_moves: Vec<MoveInstance>,
    /// Needed for handling two-turn moves.
    pub next_move_action: Option<Action>
}

impl Pokemon {
    pub const fn stat_stage(&self, stat_index: StatIndex) -> i8 {
        self.stat_stages[stat_index.as_usize()]
    }

    pub fn known_move(&self, move_index: usize) -> &MoveInstance {
        &self.known_moves[move_index]
    }

    pub fn is_type(&self, type_: Type) -> bool {
        self.first_type == type_ || self.second_type == type_
    }

    pub fn can_choose_move(&self, move_index: usize) -> bool {
        let move_instance = &self.known_moves[move_index];
        self.current_hp > 0 && self.field_position.is_some() && move_instance.pp > 0 && !move_instance.disabled
    }
}

impl Display for Pokemon {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}{}({}/{})", Species::name(self.species), self.gender.symbol(), self.current_hp, self.max_hp)
    }
}

#[derive(Clone, Debug)]
pub struct MoveInstance {
    move_: MoveID,
    pub pp: u8,
    pub disabled: bool
}

impl From<MoveID> for MoveInstance {
    fn from(move_: MoveID) -> Self {
        MoveInstance {
            move_,
            pp: Move::max_pp(move_),
            disabled: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Counter<T> {
    value: T,
    target: Option<T>
}

impl<T: AddAssign + PartialOrd + One + Zero> Counter<T> {
    pub fn new(target: Option<T>) -> Counter<T> {
        Counter {
            value: T::zero(),
            target
        }
    }

    /// Sets the value to zero and the target to None.
    fn clear(&mut self) {
        self.value.set_zero();
        self.target = None;
    }

    /// Sets the value to zero.
    fn zero(&mut self) {
        self.value.set_zero()
    }

    /// Returns whether the target value was reached as a result of incrementing.
    /// Clears the counter if it did.
    fn inc(&mut self) -> bool {
        self.add(T::one())
    }

    /// Returns whether the target value was reached as a result of the addition.
    /// Clears the counter if it did.
    fn add(&mut self, amount: T) -> bool {
        self.value += amount;

        if let Some(target) = &self.target {
            if self.value >= *target {
                self.clear();
                return true;
            }
        }

        false
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
    /// Defines how the action queue should be sorted.
    pub fn action_queue_ordering(state: &State, rng: &mut StdRng, act1: &Action, act2: &Action) -> Ordering {
        match act1 {
            Action::Move {user_id: user_id1, move_: move1, move_index: _, target_positions: _} => {
                match act2 {
                    Action::Move {user_id: user_id2, move_: move2, move_index: _, target_positions: _} => {
                        let priority_stage1 = Move::priority_stage(*move1);
                        let priority_stage2 = Move::priority_stage(*move2);
                        let priority_stage_ord = priority_stage1.cmp(&priority_stage2);
                        match priority_stage_ord {
                            Ordering::Equal => {
                                let spd1 = pokemon::calculated_stat(state, *user_id1, StatIndex::Spd);
                                let spd2 = pokemon::calculated_stat(state, *user_id2, StatIndex::Spd);
                                let spd_ord = spd1.cmp(&spd2);
                                match spd_ord {
                                    Ordering::Equal => if rng.gen_bool(0.5) { Ordering::Less } else { Ordering::Greater },
                                    _ => spd_ord.reverse()
                                }
                            },
                            _ => priority_stage_ord.reverse()
                        }
                    },
                    _ => Ordering::Greater
                }
            },
            Action::Nop => {
                match act2 {
                    Action::Nop => Ordering::Equal,
                    _ => Ordering::Less
                }
            },
            Action::Switch { .. } => {
                match act2 {
                    Action::Move { .. } => Ordering::Less,
                    Action::Nop => Ordering::Greater,
                    Action::Switch { .. } => if rng.gen_bool(0.5) { Ordering::Less } else { Ordering::Greater }
                }
            }
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

                if state.pokemon_by_id(*user_id).is_infatuated && rng.gen_bool(0.5) {
                    if cfg!(feature = "print-battle") {
                        let user_display_text = format!("{}", state.pokemon_by_id(*user_id));
                        state.add_display_text(format!("{} is infatuated with the foe!", user_display_text));
                    }
                    return false;
                }

                let user = state.pokemon_by_id(*user_id);
                if user.current_hp() == 0 || user.field_position() == None { return false; }
                match move_index {
                    Some(move_index) => {
                        let move_instance = user.known_move(*move_index as usize);
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
                        state.min.on_field
                    } else {
                        state.max.on_field
                    };

                    match target_id {
                        Some(target_id) => {
                            if cfg!(feature = "print-battle") {
                                let target_display_text = format!("{}", state.pokemon_by_id(target_id));
                                state.add_display_text(format!("- {}", target_display_text));
                            }

                            if Move::accuracy(*move_id).do_accuracy_check(state, *user_id, target_id, rng) {
                                for effect in Move::effects(*move_id) {
                                    let result = effect.do_effect(*move_id, state, action_queue, *user_id, target_id, rng);
                                    if result.has_display_text() {
                                        state.add_display_text(result.display_text().to_owned());
                                    }
                                    if state.has_battle_ended() { return true; }
                                    if state.pokemon_by_id(*user_id).current_hp() == 0 || state.pokemon_by_id(target_id).current_hp() == 0 || result == EffectResult::Fail {
                                        break;
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

// TODO: Make better; order actions so that pruning is most likely to occur.
fn action_cmp(act1: &Action, act2: &Action) -> Ordering {
    match act1 {
        Action::Nop => Ordering::Greater,
        Action::Switch { .. } => {
            match act2 {
                Action::Nop => Ordering::Less,
                Action::Switch { .. } => Ordering::Equal,
                Action::Move { .. } => Ordering::Greater
            }
        }
        Action::Move { user_id: _, move_: act1_move, move_index: _, target_positions: _ } => {
            match act2 {
                Action::Move { user_id: _, move_: act2_move, move_index: _, target_positions: _ } => {
                    match Move::category(*act1_move) {
                        MoveCategory::Status => {
                            match Move::category(*act2_move) {
                                MoveCategory::Status => Ordering::Equal,
                                _ => Ordering::Greater
                            }
                        }
                        _ => {
                            match Move::category(*act2_move) {
                                MoveCategory::Status => Ordering::Less,
                                _ => Ordering::Equal
                            }
                        }
                    }
                }
                _ => Ordering::Less
            }
        }
    }
}

fn play_out_turn(state: &mut State, mut action_queue: Vec<&Action>, rng: &mut StdRng) {
    // Only advance turn counter if all agents are actually doing something
    if !action_queue.iter().any(|act| matches!(act, Action::Nop)) {
        if cfg!(feature = "print-battle") {
            let turn_number = state.turn_number;
            state.add_display_text(format!("---- Turn {} ----", turn_number));
        }

        for id in 0..12 {
            pokemon::increment_msa_counter(state, id);
        }

        state.turn_number += 1;
        if state.weather_counter.inc() {
            state.add_display_text(String::from(state.weather.display_text_on_disappearance()));
            state.weather = Weather::None;
        }
    }

    action_queue.sort_unstable_by(|act1, act2| Action::action_queue_ordering(state, rng, act1, act2));

    while !action_queue.is_empty() {
        let action = action_queue.remove(0);
        if action.can_be_performed(state, rng) && action.perform(state, &action_queue, rng) {
            return;
        }
    }

    // End of turn effects (order is randomized to avoid bias)
    let pokemon_on_field = if rng.gen_bool(0.5) {
        vec![state.min.on_field, state.max.on_field]
    } else {
        vec![state.max.on_field, state.min.on_field]
    };

    for on_field in pokemon_on_field {
        if let Some(on_field) = on_field {
            match state.pokemon[on_field as usize].major_status_ailment() {
                MajorStatusAilment::Poisoned => {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{} takes damage from poison!", state.pokemon[on_field as usize]);
                        state.add_display_text(display_text);
                    }
                    if pokemon::apply_damage(state, on_field, max(state.pokemon[on_field as usize].max_hp() / 8, 1) as i16) {
                        return;
                    }
                }
                MajorStatusAilment::BadlyPoisoned => {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{} takes damage from poison!", state.pokemon[on_field as usize]);
                        state.add_display_text(display_text);
                    }
                    let amount = {
                        let pokemon = state.pokemon_by_id(on_field);
                        ((pokemon.msa_counter.value + 1) * max(pokemon.max_hp() / 16, 1)) as i16
                    };
                    if pokemon::apply_damage(state, on_field, amount) {
                        return;
                    }
                }
                _ => {}
            }

            if let Some(seeder_pos) = state.pokemon[on_field as usize].seeded_by {
                let seeder_id = match seeder_pos {
                    FieldPosition::Min => state.min.on_field,
                    FieldPosition::Max => state.max.on_field
                };
                if let Some(seeder_id) = seeder_id {
                    if cfg!(feature = "print-battle") {
                        let display_text = format!("{}'s seed drains energy from {}!", state.pokemon[seeder_id as usize], state.pokemon[on_field as usize]);
                        state.add_display_text(display_text);
                    }
                    let transferred_hp = max(state.pokemon[on_field as usize].max_hp() / 8, 1) as i16;
                    if pokemon::apply_damage(state, on_field, transferred_hp) || pokemon::apply_damage(state, seeder_id, -transferred_hp) {
                        return;
                    }
                }
            }
        }
    }
}

pub fn calculated_stat(state: &State, pokemon_id: u8, stat_index: StatIndex) -> u32 {
    let pokemon = state.pokemon_by_id(pokemon_id);

    if stat_index == StatIndex::Hp { return pokemon.max_hp as u32; }

    let b = Species::base_stat(pokemon.species, stat_index) as u32;
    let i = pokemon.ivs[stat_index.as_usize()] as u32;
    let e = pokemon.evs[stat_index.as_usize()] as u32;
    let mut calculated_stat = ((2 * b + i + e / 4 + 5) as f64 * pokemon.nature.stat_mod(stat_index)) as u32;

    if stat_index == StatIndex::Spd {
        if pokemon.major_status_ailment == MajorStatusAilment::Paralyzed {
            calculated_stat /= if game_version().gen() <= 6 { 4 } else { 2 };
        }
        if pokemon.ability == Ability::id_by_name("Chlorophyll").unwrap() && state.weather == Weather::HarshSunshine { calculated_stat *= 2; }
    }

    calculated_stat
}

pub fn add_to_field(state: &mut State, pokemon_id: u8, field_position: FieldPosition) -> bool {
    state.pokemon_by_id_mut(pokemon_id).field_position = Some(field_position);

    if cfg!(feature = "print-battle") {
        let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
        state.add_display_text(format!("Adding {} to field position {:?}.", pokemon_display_text, field_position));
    }
    match field_position {
        FieldPosition::Min => {
            match state.min.on_field {
                None => { state.min.on_field = Some(pokemon_id); }
                Some(min_pokemon_id) => {
                    let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
                    panic!("Tried to add {} to position {:?} occupied by {}", pokemon_display_text, field_position, state.pokemon_by_id(min_pokemon_id));
                }
            }
        }
        FieldPosition::Max => {
            match state.max.on_field {
                None => { state.max.on_field = Some(pokemon_id); }
                Some(max_pokemon_id) => {
                    let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
                    panic!("Tried to add {} to position {:?} occupied by {}", pokemon_display_text, field_position, state.pokemon_by_id(max_pokemon_id));
                }
            }
        }
    }

    state.has_battle_ended()
}

pub fn remove_from_field(state: &mut State, pokemon_id: u8) {
    remove_minor_status_ailments(state, pokemon_id);

    let old_field_pos;
    {
        let pokemon = state.pokemon_by_id_mut(pokemon_id);
        old_field_pos = pokemon.field_position.unwrap();
        pokemon.stat_stages = [0; 8];
        if game_version().gen() == 3 {
            pokemon.snore_sleep_talk_counter = 0;
        } else if game_version().gen() == 5 && pokemon.major_status_ailment == MajorStatusAilment::Asleep {
            pokemon.msa_counter.zero();
        }
        pokemon.field_position = None;
        for move_instance in &mut pokemon.known_moves {
            move_instance.disabled = false;
        }
        pokemon.next_move_action = None;
    }

    if cfg!(feature = "print-battle") {
        let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
        state.add_display_text(format!("Removing {} from field position {:?}.", pokemon_display_text, old_field_pos));
    }

    if state.min.on_field == Some(pokemon_id) {
        state.min.on_field = None;
    } else if state.max.on_field == Some(pokemon_id) {
        state.max.on_field = None;
    } else {
        let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
        panic!("ID of {} does not match any ID on the field.", pokemon_display_text);
    }
}

pub fn increment_stat_stage(state: &mut State, pokemon_id: u8, stat_index: StatIndex, requested_amount: i8) {
    let old_stat_stage;
    let new_stat_stage;
    {
        let pokemon = state.pokemon_by_id_mut(pokemon_id);
        old_stat_stage = pokemon.stat_stages[stat_index.as_usize()];
        new_stat_stage = num::clamp(old_stat_stage + requested_amount, -6, 6);
        pokemon.stat_stages[stat_index.as_usize()] = new_stat_stage;
    }

    if cfg!(feature = "print-battle") {
        let species_name = Species::name(state.pokemon_by_id(pokemon_id).species);
        let actual_change = new_stat_stage - old_stat_stage;
        match actual_change {
            c if c <= -3 => state.add_display_text(format!("{}'s {} severely fell!", species_name, stat_index.name())),
            -2 => state.add_display_text(format!("{}'s {} harshly fell!", species_name, stat_index.name())),
            -1 => state.add_display_text(format!("{}'s {} fell!", species_name, stat_index.name())),
            0 => state.add_display_text(format!("{}'s {} won't go any {}!", species_name, stat_index.name(), if requested_amount < 0 { "lower" } else { "higher" })),
            1 => state.add_display_text(format!("{}'s {} rose!", species_name, stat_index.name())),
            2 => state.add_display_text(format!("{}'s {} rose sharply!", species_name, stat_index.name())),
            _ => state.add_display_text(format!("{}'s {} rose drastically!", species_name, stat_index.name()))
        }
    }
}

/// Returns whether the poisoning was successful.
pub fn poison(state: &mut State, pokemon_id: u8, toxic: bool, corrosion: bool) -> EffectResult {
    let pokemon = state.pokemon_by_id_mut(pokemon_id);

    if !corrosion && (pokemon.is_type(Type::Poison) || pokemon.is_type(Type::Steel)) {
        return EffectResult::NoEffect;
    }

    if pokemon.major_status_ailment() == MajorStatusAilment::Okay {
        pokemon.major_status_ailment = if toxic { MajorStatusAilment::BadlyPoisoned } else { MajorStatusAilment::Poisoned };
        pokemon.msa_counter.clear();
        if cfg!(feature = "print-battle") {
            let species_name = Species::name(state.pokemon_by_id(pokemon_id).species);
            state.add_display_text(format!("{}{}", species_name, if toxic { MajorStatusAilment::BadlyPoisoned.display_text_when_applied() } else { MajorStatusAilment::Poisoned.display_text_when_applied() }));
        }
        return EffectResult::Success;
    }

    EffectResult::Fail
}

/// Returns whether the Pokemon fell asleep.
pub fn put_to_sleep(state: &mut State, pokemon_id: u8, rng: &mut StdRng) -> EffectResult {
    let pokemon = state.pokemon_by_id_mut(pokemon_id);

    if pokemon.major_status_ailment() == MajorStatusAilment::Okay {
        pokemon.major_status_ailment = MajorStatusAilment::Asleep;
        pokemon.msa_counter = Counter::new(Some(
            match game_version().gen() {
                1 => rng.gen_range(1, 7),
                2 => rng.gen_range(1, 5),
                3..=4 => rng.gen_range(2, 5),
                _ => rng.gen_range(1, 3)
            }
        ));
        if cfg!(feature = "print-battle") {
            let species_name = Species::name(state.pokemon_by_id(pokemon_id).species);
            state.add_display_text(format!("{}{}", species_name, MajorStatusAilment::Asleep.display_text_when_applied()));
        }
        return EffectResult::Success;
    }

    EffectResult::Fail
}

pub fn increment_msa_counter(state: &mut State, pokemon_id: u8) {
    let mut msa_cured = false;
    let mut old_msa = MajorStatusAilment::Okay;
    {
        let pokemon = state.pokemon_by_id_mut(pokemon_id);
        if pokemon.msa_counter.add(pokemon.snore_sleep_talk_counter + 1) {
            msa_cured = true;
            old_msa = pokemon.major_status_ailment;
            pokemon.major_status_ailment = MajorStatusAilment::Okay;
        }
        pokemon.snore_sleep_talk_counter = 0;
    }

    if msa_cured && cfg!(feature = "print-battle") {
        let species_name = Species::name(state.pokemon_by_id(pokemon_id).species);
        state.add_display_text(format!("{}{}", species_name, old_msa.display_text_when_cured()));
    }
}

/// The amount can be negative to add HP.
pub fn apply_damage(state: &mut State, pokemon_id: u8, amount: i16) -> bool {
    let new_hp = state.pokemon_by_id(pokemon_id).current_hp as i16 - amount;
    if new_hp <= 0 {
        state.pokemon_by_id_mut(pokemon_id).current_hp = 0;
        if cfg!(feature = "print-battle") {
            let display_text = format!("{} fainted!", state.pokemon_by_id(pokemon_id));
            state.add_display_text(display_text);
        }
        remove_from_field(state, pokemon_id);
        return state.has_battle_ended();
    }

    let pokemon = state.pokemon_by_id_mut(pokemon_id);
    pokemon.current_hp = min(new_hp as u16, pokemon.max_hp);
    false
}

pub fn increment_move_pp(state: &mut State, pokemon_id: u8, move_index: u8, amount: i8) {
    let move_instance = &mut state.pokemon_by_id_mut(pokemon_id).known_moves[move_index as usize];
    move_instance.pp = num::clamp(move_instance.pp as i8 + amount, 0, Move::max_pp(move_instance.move_) as i8) as u8;
}

fn remove_minor_status_ailments(state: &mut State, pokemon_id: u8) {
    let pokemon = state.pokemon_by_id_mut(pokemon_id);
    pokemon.confusion_counter.clear();
    pokemon.is_flinching = false;
    pokemon.seeded_by = None;
    pokemon.is_infatuated = false;
    pokemon.is_cursed = false;
    pokemon.has_nightmare = false;
}

pub fn set_infatuated(state: &mut State, pokemon_id: u8, caused_by: u8) {
    let pokemon_name = Species::name(state.pokemon_by_id(pokemon_id).species());
    let caused_name = Species::name(state.pokemon_by_id(caused_by).species());
    state.add_display_text(format!("{} became infatuated with {}!", pokemon_name, caused_name));
    state.pokemon_by_id_mut(pokemon_id).is_infatuated = true;
}
