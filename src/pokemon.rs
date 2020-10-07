use crate::species::Species;
use crate::{Type, Gender, Nature, Ability, MajorStatusAilment, FieldPosition, StatIndex, game_version, Weather, clamp};
use crate::state::StateSpace;
use std::fmt::{Display, Formatter, Error};
use rand::Rng;
use crate::move_::{MoveAction, Move};

#[derive(Clone, Debug)]
pub struct Pokemon {
    pub species: &'static Species,
    pub first_type: Type, // Types usually match the species' type, but some Pokemon can change types
    pub second_type: Type,
    gender: Gender,
    nature: Nature,
    pub ability: Ability,
    ivs: [u8; 6],
    evs: [u8; 6],
    pub max_hp: u16,
    pub current_hp: u16,
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
    pub seeded_by: Option<u8>, // ID of the Pokemon that seeded this Pokemon
    is_infatuated: bool,
    is_cursed: bool,
    has_nightmare: bool,

    pub field_position: Option<FieldPosition>,

    // Moves
    pub known_moves: Vec<MoveInstance>,
    pub next_move_action: Option<MoveAction> // Needed for handling two-turn moves
}

impl Pokemon {
    fn new(species: &'static Species, gender: Gender, nature: Nature, ability: Ability, ivs: [u8; 6], evs: [u8; 6], known_moves: Vec<&'static Move>) -> Pokemon {
        let max_hp = (2 * species.base_stat(StatIndex::Hp) + ivs[StatIndex::Hp.as_usize()] + evs[StatIndex::Hp.as_usize()] / 4 + 110) as u16;
        Pokemon {
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
            known_moves: known_moves.iter().map(|move_| MoveInstance::new(move_)).collect(),
            next_move_action: None
        }
    }

    pub const fn is_type(&self, type_: Type) -> bool {
        self.first_type == type_ || self.second_type == type_
    }

    // TODO: Can move_index be usize instead of Option?
    pub const fn can_choose_move(&self, move_index: Option<usize>) -> bool {
        if self.current_hp == 0 || self.field_position == None { return false; }
        match move_index {
            Some(move_index) => {
                let move_instance = self.known_moves.get(move_index).unwrap();
                move_instance.pp > 0 && !move_instance.disabled
            },
            None => true
        }
    }

    pub const fn stat_stage(&self, stat_index: StatIndex) -> i8 {
        self.stat_stages[stat_index.as_usize()]
    }

    pub const fn major_status_ailment(&self) -> MajorStatusAilment {
        self.major_status_ailment
    }
}

impl Display for Pokemon {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}{}({}/{})", self.species.name, self.gender.symbol(), self.current_hp, self.max_hp)
    }
}

#[derive(Clone, Debug)]
struct MoveInstance {
    pub move_: &'static Move,
    pub pp: u8,
    pub disabled: bool
}

impl MoveInstance {
    fn new(move_: &'static Move) -> MoveInstance {
        MoveInstance {
            move_,
            pp: move_.max_pp,
            disabled: false
        }
    }
}

pub const fn calculated_stat(state_space: &StateSpace, state_id: usize, pokemon_id: u8, stat_index: StatIndex) -> u32 {
    let pokemon = state_space.get(state_id).pokemon_by_id(pokemon_id);

    if stat_index == StatIndex::Hp { return pokemon.max_hp as u32; }

    let b = pokemon.species.base_stat(stat_index) as u32;
    let i = pokemon.ivs[stat_index.as_usize()] as u32;
    let e = pokemon.evs[stat_index.as_usize()] as u32;
    let mut calculated_stat = ((2 * b + i + e / 4 + 5) as f64 * pokemon.nature.stat_mod(stat_index)) as u32;

    if stat_index == StatIndex::Spd {
        if pokemon.major_status_ailment == MajorStatusAilment::Paralyzed {
            calculated_stat /= if game_version().gen() <= 6 { 4 } else { 2 };
        }
        if pokemon.ability == Ability::Chlorophyll && state_space.get(state_id).weather == Weather::Sunshine { calculated_stat *= 2; }
    }

    calculated_stat
}

pub const fn add_to_field(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8, field_position: FieldPosition) -> bool {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    state.display_text.push(format!("Adding {} to field position {:?}.", pokemon, field_position));
    pokemon.field_position = Some(field_position);
    match field_position {
        FieldPosition::Min => {
            match state.min_pokemon_id {
                None => { state.min_pokemon_id = Some(pokemon_id); },
                Some(min_pokemon_id) => {
                    panic!(format!("Tried to add {} to position {:?} occupied by {}", pokemon, field_position, state.pokemon_by_id(min_pokemon_id)));
                }
            }
        },
        FieldPosition::Max => {
            match state.max_pokemon_id {
                None => { state.max_pokemon_id = Some(pokemon_id); },
                Some(max_pokemon_id) => {
                    panic!(format!("Tried to add {} to position {:?} occupied by {}", pokemon, field_position, state.pokemon_by_id(max_pokemon_id)));
                }
            }
        }
    }

    state.battle_end_check()
}

fn remove_from_field(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8) {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    state.display_text.push(format!("Removing {} from field position {:?}.", pokemon, pokemon.field_position));
    pokemon.stat_stages = [0; 8];
    remove_minor_status_ailments(state_space, state_id, pokemon_id);
    if game_version().gen() == 3 { pokemon.snore_sleep_talk_counter = 0; }
    if game_version().gen() == 5 && pokemon.major_status_ailment == MajorStatusAilment::Asleep { pokemon.msa_counter = 0; }
    pokemon.field_position = None;
    for mut move_instance in pokemon.known_moves {
        move_instance.disabled = false;
    }
    pokemon.next_move_action = None;

    let min_pokemon: Option<&mut Pokemon> = state.min_pokemon_id.map(|id| state.pokemon_by_id_mut(id));

    if let Some(min_pokemon) = min_pokemon {
        if let Some(seeder_id) = min_pokemon.seeded_by {
            if seeder_id == pokemon_id { min_pokemon.seeded_by = None; }
        }
    }
    let max_pokemon: Option<&mut Pokemon> = state.max_pokemon_id.map(|id| state.pokemon_by_id_mut(id));
    if let Some(max_pokemon) = max_pokemon {
        if let Some(seeder_id) = max_pokemon.seeded_by {
            if seeder_id == pokemon_id { max_pokemon.seeded_by = None; }
        }
    }

    if state.min_pokemon_id == Some(pokemon_id) {
        state.min_pokemon_id = None;
    } else if state.max_pokemon_id == Some(pokemon_id) {
        state.max_pokemon_id = None;
    } else {
        panic!(format!("ID of {} does not match any ID on the field.", pokemon));
    }
}

const fn increment_stat_stage(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8, stat_index: StatIndex, requested_amount: i8) {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    let old_stat_stage = pokemon.stat_stages[stat_index.as_usize()];
    let new_stat_stage = clamp(old_stat_stage + requested_amount, -6, 6);
    pokemon.stat_stages[stat_index.as_usize()] = new_stat_stage;
    let actual_change = new_stat_stage - old_stat_stage;
    if actual_change <= -3 {
        state.display_text.push(format!("{}'s {} severely fell!", pokemon.species.name, stat_index.name()));
    } else if actual_change == -2 {
        state.display_text.push(format!("{}'s {} harshly fell!", pokemon.species.name, stat_index.name()));
    } else if actual_change == -1 {
        state.display_text.push(format!("{}'s {} fell!", pokemon.species.name, stat_index.name()));
    } else if actual_change == 0 {
        state.display_text.push(format!("{}'s {} won't go any {}!", pokemon.species.name, stat_index.name(), if requested_amount < 0 { "lower" } else { "higher" }));
    } else if actual_change == 1 {
        state.display_text.push(format!("{}'s {} rose!", pokemon.species.name, stat_index.name()));
    } else if actual_change == 2 {
        state.display_text.push(format!("{}'s {} rose sharply!", pokemon.species.name, stat_index.name()));
    } else {
        state.display_text.push(format!("{}'s {} rose drastically!", pokemon.species.name, stat_index.name()));
    }
}

pub const fn increment_msa_counter(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8) {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    if let Some(msa_counter_target) = pokemon.msa_counter_target {
        if pokemon.major_status_ailment != MajorStatusAilment::Okay {
            pokemon.msa_counter += pokemon.snore_sleep_talk_counter + 1;
            pokemon.snore_sleep_talk_counter = 0;
            if pokemon.msa_counter >= msa_counter_target {
                state.display_text.push(format!("{}{}", pokemon.species.name, pokemon.major_status_ailment.display_text_when_cured()));
                pokemon.major_status_ailment = MajorStatusAilment::Okay;
                pokemon.msa_counter = 0;
                pokemon.msa_counter_target = None;
            }
        }
    }
}

const fn increment_snore_sleep_talk_counter(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8) {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    if pokemon.major_status_ailment != MajorStatusAilment::Asleep { panic!("snore_sleep_talk_counter incremented while not asleep"); }
    if let Some(msa_counter_target) = pokemon.msa_counter_target {
        if game_version().gen() == 3 {
            pokemon.snore_sleep_talk_counter += 1;
            if pokemon.snore_sleep_talk_counter >= msa_counter_target {
                state.display_text.push(format!("{}{}", pokemon.species.name, MajorStatusAilment::Asleep.display_text_when_cured()));
                pokemon.major_status_ailment = MajorStatusAilment::Okay;
                pokemon.snore_sleep_talk_counter = 0;
                pokemon.msa_counter = 0;
                pokemon.msa_counter_target = None;
            }
        }
    }
}

/// Returns true if the poisoning was successful.
const fn inflict_poison(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8) -> bool {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    if pokemon.major_status_ailment == MajorStatusAilment::Okay && !pokemon.is_type(Type::Poison) && !pokemon.is_type(Type::Steel) {
        pokemon.major_status_ailment = MajorStatusAilment::Poisoned;
        pokemon.msa_counter = 0;
        pokemon.msa_counter_target = None;
        state.display_text.push(format!("{} was poisoned!", pokemon.species.name));
        return true;
    }

    false
}

/// Returns true if putting this Pokemon to sleep was successful.
const fn inflict_sleep(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8) -> bool {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    if pokemon.major_status_ailment == MajorStatusAilment::Okay {
        pokemon.major_status_ailment = MajorStatusAilment::Asleep;
        pokemon.msa_counter = 0;
        // TODO: Use seeded RNG
        pokemon.msa_counter_target = Some(if game_version().gen() <= 4 { rand::thread_rng().gen_range(2, 6) } else { rand::thread_rng().gen_range(1, 4) });
        if game_version().gen() == 3 { pokemon.snore_sleep_talk_counter = 0; }
        state.display_text.push(format!("{} fell asleep!", pokemon.species.name));
        return true;
    }

    false
}

/// The amount can be negative to add HP.
pub const fn apply_damage(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8, amount: i16) -> bool {
    let state = state_space.get_mut(state_id);
    let pokemon: &mut Pokemon = state.pokemon_by_id_mut(pokemon_id);

    let new_hp = pokemon.current_hp as i16 - amount;
    if new_hp <= 0 {
        pokemon.current_hp = 0;
        state.display_text.push(format!("{} fainted!", pokemon));
        remove_from_field(state_space, state_id, pokemon_id);
        return state.battle_end_check();
    }
    pokemon.current_hp = new_hp as u16;
    if pokemon.current_hp > pokemon.max_hp {
        pokemon.current_hp = pokemon.max_hp;
    }
    false
}

const fn remove_minor_status_ailments(state_space: &mut StateSpace, state_id: usize, pokemon_id: u8) {
    let pokemon: &mut Pokemon = state_space.get_mut(state_id).pokemon_by_id_mut(pokemon_id);
    pokemon.confusion_turn_inflicted = None;
    pokemon.confusion_turn_will_cure = None;
    pokemon.is_flinching = false;
    pokemon.seeded_by = None;
    pokemon.is_infatuated = false;
    pokemon.is_cursed = false;
    pokemon.has_nightmare = false;
}
