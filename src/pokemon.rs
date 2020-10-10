use std::cmp::min;
use std::fmt::{Display, Error, Formatter};

use crate::{Ability, clamp, FieldPosition, game_version, Gender, MajorStatusAilment, Nature, StatIndex, Type, Weather};
use crate::move_::{MoveActionV2, MoveV2};
use crate::species::SpeciesV2;
use crate::state::StateV2;

#[derive(Clone, Debug)]
pub struct PokemonV2 {
    pub species: &'static SpeciesV2,
    pub first_type: Type,
    // Types usually match the species' type, but some Pokemon can change types
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
    msa_counter: u16,
    msa_counter_target: Option<u16>,
    snore_sleep_talk_counter: u8, // Only used in gen 3; otherwise it's always 0

    // Minor status ailments
    confusion_turn_inflicted: Option<u16>,
    // Turn on which confusion was inflicted
    confusion_turn_will_cure: Option<u16>,
    // Turn on which confusion will be cured naturally
    is_flinching: bool,
    pub seeded_by: Option<u8>,
    // ID of the Pokemon that seeded this Pokemon
    is_infatuated: bool,
    is_cursed: bool,
    has_nightmare: bool,

    pub field_position: Option<FieldPosition>,

    // Moves
    pub known_moves: Vec<MoveInstanceV2>,
    pub next_move_action: Option<MoveActionV2>, // Needed for handling two-turn moves
}

impl PokemonV2 {
    pub fn new(species: &'static SpeciesV2, gender: Gender, nature: Nature, ability: Ability, ivs: [u8; 6], evs: [u8; 6], known_moves: &[&'static MoveV2]) -> PokemonV2 {
        let max_hp = (2 * species.base_stat(StatIndex::Hp) + ivs[StatIndex::Hp.as_usize()] + evs[StatIndex::Hp.as_usize()] / 4 + 110) as u16;
        PokemonV2 {
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
            known_moves: known_moves.iter().map(|move_| MoveInstanceV2::new(move_)).collect(),
            next_move_action: None,
        }
    }

    pub fn is_type(&self, type_: Type) -> bool {
        self.first_type == type_ || self.second_type == type_
    }

    // TODO: Can move_index be usize instead of Option?
    pub fn can_choose_move(&self, move_index: Option<usize>) -> bool {
        if self.current_hp == 0 || self.field_position == None { return false; }
        match move_index {
            Some(move_index) => {
                let move_instance = self.known_moves.get(move_index).unwrap();
                move_instance.pp > 0 && !move_instance.disabled
            }
            None => true
        }
    }

    pub fn stat_stage(&self, stat_index: StatIndex) -> i8 {
        self.stat_stages[stat_index.as_usize()]
    }

    pub const fn major_status_ailment(&self) -> MajorStatusAilment {
        self.major_status_ailment
    }
}

impl Display for PokemonV2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}{}({}/{})", self.species.name, self.gender.symbol(), self.current_hp, self.max_hp)
    }
}

#[derive(Clone, Debug)]
pub struct MoveInstanceV2 {
    pub move_: &'static MoveV2,
    pub pp: u8,
    pub disabled: bool,
}

impl MoveInstanceV2 {
    fn new(move_: &'static MoveV2) -> MoveInstanceV2 {
        MoveInstanceV2 {
            move_,
            pp: move_.max_pp,
            disabled: false,
        }
    }
}

pub fn calculated_stat_v2(state_box: &Box<StateV2>, pokemon_id: u8, stat_index: StatIndex) -> u32 {
    let pokemon = &state_box.pokemon[pokemon_id as usize];

    if stat_index == StatIndex::Hp { return pokemon.max_hp as u32; }

    let b = pokemon.species.base_stat(stat_index) as u32;
    let i = pokemon.ivs[stat_index.as_usize()] as u32;
    let e = pokemon.evs[stat_index.as_usize()] as u32;
    let mut calculated_stat = ((2 * b + i + e / 4 + 5) as f64 * pokemon.nature.stat_mod(stat_index)) as u32;

    if stat_index == StatIndex::Spd {
        if pokemon.major_status_ailment == MajorStatusAilment::Paralyzed {
            calculated_stat /= if game_version().gen() <= 6 { 4 } else { 2 };
        }
        if pokemon.ability == Ability::Chlorophyll && state_box.weather == Weather::Sunshine { calculated_stat *= 2; }
    }

    calculated_stat
}

pub fn add_to_field_v2(state_box: &mut Box<StateV2>, pokemon_id: u8, field_position: FieldPosition) -> bool {
    state_box.pokemon[pokemon_id as usize].field_position = Some(field_position);

    if cfg!(feature = "print-battle") {
        let pokemon_display_text = format!("{}", state_box.pokemon[pokemon_id as usize]);
        state_box.display_text.push(format!("Adding {} to field position {:?}.", pokemon_display_text, field_position));
    }
    match field_position {
        FieldPosition::Min => {
            match state_box.min_pokemon_id {
                None => { state_box.min_pokemon_id = Some(pokemon_id); }
                Some(min_pokemon_id) => {
                    let pokemon_display_text = format!("{}", state_box.pokemon[pokemon_id as usize]);
                    panic!(format!("Tried to add {} to position {:?} occupied by {}", pokemon_display_text, field_position, state_box.pokemon[min_pokemon_id as usize]));
                }
            }
        }
        FieldPosition::Max => {
            match state_box.max_pokemon_id {
                None => { state_box.max_pokemon_id = Some(pokemon_id); }
                Some(max_pokemon_id) => {
                    let pokemon_display_text = format!("{}", state_box.pokemon[pokemon_id as usize]);
                    panic!(format!("Tried to add {} to position {:?} occupied by {}", pokemon_display_text, field_position, state_box.pokemon[max_pokemon_id as usize]));
                }
            }
        }
    }

    state_box.battle_end_check()
}

fn remove_from_field_v2(state_box: &mut Box<StateV2>, pokemon_id: u8) {
    remove_minor_status_ailments_v2(state_box, pokemon_id);

    let old_field_pos;
    {
        let pokemon = &mut state_box.pokemon[pokemon_id as usize];
        old_field_pos = pokemon.field_position.unwrap();
        pokemon.stat_stages = [0; 8];
        if game_version().gen() == 3 { pokemon.snore_sleep_talk_counter = 0; }
        if game_version().gen() == 5 && pokemon.major_status_ailment == MajorStatusAilment::Asleep { pokemon.msa_counter = 0; }
        pokemon.field_position = None;
        for move_instance in &mut pokemon.known_moves {
            move_instance.disabled = false;
        }
        pokemon.next_move_action = None;
    }

    if cfg!(feature = "print-battle") {
        let pokemon_display_text = format!("{}", state_box.pokemon[pokemon_id as usize]);
        state_box.display_text.push(format!("Removing {} from field position {:?}.", pokemon_display_text, old_field_pos));
    }

    if let Some(min_pokemon_id) = state_box.min_pokemon_id {
        let min_pokemon = &mut state_box.pokemon[min_pokemon_id as usize];
        if let Some(seeder_id) = min_pokemon.seeded_by {
            if seeder_id == pokemon_id { min_pokemon.seeded_by = None; }
        }
    }
    if let Some(max_pokemon_id) = state_box.max_pokemon_id {
        let max_pokemon = &mut state_box.pokemon[max_pokemon_id as usize];
        if let Some(seeder_id) = max_pokemon.seeded_by {
            if seeder_id == pokemon_id { max_pokemon.seeded_by = None; }
        }
    }

    if state_box.min_pokemon_id == Some(pokemon_id) {
        state_box.min_pokemon_id = None;
    } else if state_box.max_pokemon_id == Some(pokemon_id) {
        state_box.max_pokemon_id = None;
    } else {
        let pokemon_display_text = format!("{}", state_box.pokemon[pokemon_id as usize]);
        panic!(format!("ID of {} does not match any ID on the field.", pokemon_display_text));
    }
}

pub fn increment_stat_stage_v2(state_box: &mut Box<StateV2>, pokemon_id: u8, stat_index: StatIndex, requested_amount: i8) {
    let old_stat_stage;
    let new_stat_stage;
    {
        let pokemon = &mut state_box.pokemon[pokemon_id as usize];
        old_stat_stage = pokemon.stat_stages[stat_index.as_usize()];
        new_stat_stage = clamp(old_stat_stage + requested_amount, -6, 6);
        pokemon.stat_stages[stat_index.as_usize()] = new_stat_stage;
    }

    if cfg!(feature = "print-battle") {
        let pokemon_species_name = state_box.pokemon[pokemon_id as usize].species.name;
        let actual_change = new_stat_stage - old_stat_stage;
        match actual_change {
            c if c <= -3 => state_box.display_text.push(format!("{}'s {} severely fell!", pokemon_species_name, stat_index.name())),
            -2 => state_box.display_text.push(format!("{}'s {} harshly fell!", pokemon_species_name, stat_index.name())),
            -1 => state_box.display_text.push(format!("{}'s {} fell!", pokemon_species_name, stat_index.name())),
            0 => state_box.display_text.push(format!("{}'s {} won't go any {}!", pokemon_species_name, stat_index.name(), if requested_amount < 0 { "lower" } else { "higher" })),
            1 => state_box.display_text.push(format!("{}'s {} rose!", pokemon_species_name, stat_index.name())),
            2 => state_box.display_text.push(format!("{}'s {} rose sharply!", pokemon_species_name, stat_index.name())),
            _ => state_box.display_text.push(format!("{}'s {} rose drastically!", pokemon_species_name, stat_index.name()))
        }
    }
}

pub fn increment_msa_counter_v2(state_box: &mut Box<StateV2>, pokemon_id: u8) {
    let mut msa_cured = false;
    {
        let pokemon = &mut state_box.pokemon[pokemon_id as usize];
        if let Some(msa_counter_target) = pokemon.msa_counter_target {
            if pokemon.major_status_ailment != MajorStatusAilment::Okay {
                pokemon.msa_counter += pokemon.snore_sleep_talk_counter as u16 + 1;
                pokemon.snore_sleep_talk_counter = 0;
                if pokemon.msa_counter >= msa_counter_target {
                    msa_cured = true;
                    pokemon.major_status_ailment = MajorStatusAilment::Okay;
                    pokemon.msa_counter = 0;
                    pokemon.msa_counter_target = None;
                }
            }
        }
    }

    if msa_cured && cfg!(feature = "print-battle") {
        let pokemon = &state_box.pokemon[pokemon_id as usize];
        let pokemon_species_name = pokemon.species.name;
        let cured_display_text = pokemon.major_status_ailment.display_text_when_cured();
        state_box.display_text.push(format!("{}{}", pokemon_species_name, cured_display_text));
    }
}

/// The amount can be negative to add HP.
pub fn apply_damage_v2(state_box: &mut Box<StateV2>, pokemon_id: u8, amount: i16) -> bool {
    let new_hp = state_box.pokemon[pokemon_id as usize].current_hp as i16 - amount;
    if new_hp <= 0 {
        state_box.pokemon[pokemon_id as usize].current_hp = 0;
        if cfg!(feature = "print-battle") {
            let display_text = format!("{} fainted!", &state_box.pokemon[pokemon_id as usize]);
            state_box.display_text.push(display_text);
        }
        remove_from_field_v2(state_box, pokemon_id);
        return state_box.battle_end_check();
    }

    let pokemon = &mut state_box.pokemon[pokemon_id as usize];
    pokemon.current_hp = min(new_hp as u16, pokemon.max_hp);
    false
}

fn remove_minor_status_ailments_v2(state_box: &mut Box<StateV2>, pokemon_id: u8) {
    let pokemon = &mut state_box.pokemon[pokemon_id as usize];
    pokemon.confusion_turn_inflicted = None;
    pokemon.confusion_turn_will_cure = None;
    pokemon.is_flinching = false;
    pokemon.seeded_by = None;
    pokemon.is_infatuated = false;
    pokemon.is_cursed = false;
    pokemon.has_nightmare = false;
}
