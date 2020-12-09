use rand::prelude::StdRng;
use rand::Rng;
use serde::export::TryFrom;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use std::collections::BTreeSet;
use std::iter::FromIterator;
use crate::{StatIndex, MajorStatusAilment, Ability, game_version, FieldPosition, Weather, Type, Gender, Nature, AbilityID, clamp};
use crate::species::{SpeciesID, Species};
use crate::battle_ai::move_effects::Action;
use crate::move_::{MoveID, Move};
use crate::battle_ai::state::State;

#[derive(Clone, Debug)]
/// Assumed to be level 100.
pub struct Pokemon {
    species: SpeciesID,
    // Types usually match the species' type, but some Pokemon can change types
    first_type: Type,
    second_type: Type,
    gender: Gender,
    nature: Nature,
    ability: AbilityID,
    ivs: [u8; 6],
    evs: [u8; 6],
    max_hp: u16,
    current_hp: u16,
    stat_stages: [i8; 8],

    // Major status ailment
    major_status_ailment: MajorStatusAilment,
    msa_counter: u16,
    msa_counter_target: Option<u16>,
    /// Only used in gen 3; otherwise it's always 0.
    snore_sleep_talk_counter: u16,

    // Minor status ailments
    /// Turn on which confusion was inflicted.
    confusion_turn_inflicted: Option<u16>,
    /// Turn on which confusion will be cured naturally.
    confusion_turn_will_cure: Option<u16>,
    is_flinching: bool,
    /// ID of the Pokemon that seeded this Pokemon.
    pub seeded_by: Option<u8>,
    is_infatuated: bool,
    is_cursed: bool,
    has_nightmare: bool,

    field_position: Option<FieldPosition>,
    known_moves: Vec<MoveInstance>,
    /// Needed for handling two-turn moves.
    pub next_move_action: Option<Action>
}

impl Pokemon {
    pub const fn species(&self) -> SpeciesID {
        self.species
    }

    pub const fn first_type(&self) -> Type {
        self.first_type
    }

    pub const fn second_type(&self) -> Type {
        self.second_type
    }

    pub const fn ability(&self) -> AbilityID {
        self.ability
    }

    pub const fn max_hp(&self) -> u16 {
        self.max_hp
    }

    pub const fn current_hp(&self) -> u16 {
        self.current_hp
    }

    pub const fn stat_stage(&self, stat_index: StatIndex) -> i8 {
        self.stat_stages[stat_index.as_usize()]
    }

    pub const fn major_status_ailment(&self) -> MajorStatusAilment {
        self.major_status_ailment
    }

    pub const fn field_position(&self) -> Option<FieldPosition> {
        self.field_position
    }

    pub fn known_move(&self, move_index: u8) -> &MoveInstance {
        &self.known_moves[move_index as usize]
    }

    pub fn known_moves(&self) -> &[MoveInstance] {
        &self.known_moves
    }

    pub fn is_type(&self, type_: Type) -> bool {
        self.first_type == type_ || self.second_type == type_
    }

    // TODO: Can move_index be usize instead of Option?
    pub fn can_choose_move(&self, move_index: Option<u8>) -> bool {
        if self.current_hp == 0 || self.field_position == None { return false; }
        match move_index {
            Some(move_index) => {
                let move_instance = &self.known_moves[move_index as usize];
                move_instance.pp > 0 && !move_instance.disabled
            }
            None => true
        }
    }
}

impl From<&PokemonBuild> for Pokemon {
    fn from(pb: &PokemonBuild) -> Self {
        let max_hp = 2 * Species::base_stat(pb.species, StatIndex::Hp) as u16 + pb.ivs[StatIndex::Hp.as_usize()] as u16 + pb.evs[StatIndex::Hp.as_usize()] as u16 / 4 + 110;
        Pokemon {
            species: pb.species,
            first_type: Species::type1(pb.species),
            second_type: Species::type2(pb.species),
            gender: pb.gender,
            nature: pb.nature,
            ability: pb.ability,
            ivs: pb.ivs,
            evs: pb.ivs,
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
            known_moves: pb.moves.iter().map(|move_| MoveInstance::from(*move_)).collect(),
            next_move_action: None
        }
    }
}

impl Display for Pokemon {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{}{}({}/{})", Species::name(self.species), self.gender.symbol(), self.current_hp, self.max_hp)
    }
}

/// Part of a `TeamBuild`; contains all the necessary information to create a `Pokemon` object.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "PokemonBuildSerde", into = "PokemonBuildSerde")]
pub struct PokemonBuild {
    pub species: SpeciesID,
    pub gender: Gender,
    pub nature: Nature,
    pub ability: AbilityID,
    pub ivs: [u8; 6],
    pub evs: [u8; 6],
    /// Contains 0-4 moves.
    pub moves: BTreeSet<MoveID>
}

impl PokemonBuild {
    pub const fn num_vars() -> usize {
        20
    }

    pub fn new(rng: &mut StdRng) -> PokemonBuild {
        let species = Species::random_species(rng);
        let mut pokemon_build = PokemonBuild {
            species,
            gender: Species::random_gender(species, rng),
            nature: Nature::random_nature(rng),
            ability: Species::random_ability(species, rng),
            ivs: [rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32), rng.gen_range(0, 32)],
            evs: [rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253), rng.gen_range(0, 253)],
            moves: BTreeSet::from_iter(Species::random_move_set(species, rng)),
        };
        pokemon_build.fix_evs(rng);
        pokemon_build
    }

    pub const fn species(&self) -> SpeciesID {
        self.species
    }

    pub const fn gender(&self) -> Gender {
        self.gender
    }

    pub const fn nature(&self) -> Nature {
        self.nature
    }

    pub const fn ability(&self) -> AbilityID {
        self.ability
    }

    pub const fn ivs(&self) -> &[u8] {
        &self.ivs
    }

    pub const fn evs(&self) -> &[u8] {
        &self.evs
    }

    pub fn moves(&self) -> &BTreeSet<MoveID> {
        &self.moves
    }

    fn fix_evs(&mut self, rng: &mut StdRng) {
        let mut ev_sum: u16 = self.evs.iter().map(|ev| *ev as u16).sum();
        while ev_sum < 510 {
            let i = rng.gen_range(0, 6);
            if self.evs[i] < 252 {
                self.evs[i] += 1;
                ev_sum += 1;
            }
        }
        while ev_sum > 510 {
            let i = rng.gen_range(0, 6);
            if self.evs[i] > 0 {
                self.evs[i] -= 1;
                ev_sum -= 1;
            }
        }
    }
}

impl TryFrom<PokemonBuildSerde<'_>> for PokemonBuild {
    type Error = String;

    fn try_from(pb_serde: PokemonBuildSerde) -> Result<Self, Self::Error> {
        Ok(PokemonBuild {
            species: Species::id_by_name(pb_serde.species)?,
            gender: pb_serde.gender,
            nature: pb_serde.nature,
            ability: Ability::id_by_name(pb_serde.ability)?,
            ivs: pb_serde.ivs,
            evs: pb_serde.evs,
            moves: BTreeSet::from_iter(vec![
                Move::id_by_name(pb_serde.move1)?,
                Move::id_by_name(pb_serde.move2)?,
                Move::id_by_name(pb_serde.move3)?,
                Move::id_by_name(pb_serde.move4)?
            ])
        })
    }
}

#[derive(Deserialize, Serialize)]
struct PokemonBuildSerde<'d> {
    species: &'d str,
    gender: Gender,
    nature: Nature,
    ability: &'d str,
    ivs: [u8; 6],
    evs: [u8; 6],
    move1: &'d str,
    move2: &'d str,
    move3: &'d str,
    move4: &'d str
}

impl From<PokemonBuild> for PokemonBuildSerde<'_> {
    fn from(pokemon_build: PokemonBuild) -> Self {
        let moves: Vec<MoveID> = pokemon_build.moves.iter().copied().collect();
        PokemonBuildSerde {
            species: Species::name(pokemon_build.species),
            gender: pokemon_build.gender,
            nature: pokemon_build.nature,
            ability: Ability::name(pokemon_build.ability),
            ivs: pokemon_build.ivs,
            evs: pokemon_build.evs,
            move1: moves.get(0).map(|&move_| Move::name(move_)).unwrap_or(""),
            move2: moves.get(1).map(|&move_| Move::name(move_)).unwrap_or(""),
            move3: moves.get(2).map(|&move_| Move::name(move_)).unwrap_or(""),
            move4: moves.get(3).map(|&move_| Move::name(move_)).unwrap_or("")
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TeamBuild {
    /// The party leader is separate since they are always the first Pokemon sent out.
    /// The rest of the team can be freely switched in and out of battle.
    pub party_leader: PokemonBuild,
    /// Contains the rest of the team.
    pub remaining_team: [PokemonBuild; 5]
}

impl TeamBuild {
    pub const fn num_vars() -> usize {
        PokemonBuild::num_vars() * 6
    }

    pub fn new(rng: &mut StdRng) -> TeamBuild {
        TeamBuild {
            party_leader: PokemonBuild::new(rng),
            remaining_team: [
                PokemonBuild::new(rng),
                PokemonBuild::new(rng),
                PokemonBuild::new(rng),
                PokemonBuild::new(rng),
                PokemonBuild::new(rng)
            ]
        }
    }
}

#[derive(Clone, Debug)]
pub struct MoveInstance {
    move_: MoveID,
    pub pp: u8,
    pub disabled: bool
}

impl MoveInstance {
    pub fn move_(&self) -> MoveID {
        self.move_
    }
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
        if pokemon.ability == Ability::id_by_name("Chlorophyll").unwrap() && state.weather == Weather::Sunshine { calculated_stat *= 2; }
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
            match state.min_pokemon_id {
                None => { state.min_pokemon_id = Some(pokemon_id); }
                Some(min_pokemon_id) => {
                    let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
                    panic!(format!("Tried to add {} to position {:?} occupied by {}", pokemon_display_text, field_position, state.pokemon_by_id(min_pokemon_id)));
                }
            }
        }
        FieldPosition::Max => {
            match state.max_pokemon_id {
                None => { state.max_pokemon_id = Some(pokemon_id); }
                Some(max_pokemon_id) => {
                    let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
                    panic!(format!("Tried to add {} to position {:?} occupied by {}", pokemon_display_text, field_position, state.pokemon_by_id(max_pokemon_id)));
                }
            }
        }
    }

    state.battle_end_check()
}

pub fn remove_from_field(state: &mut State, pokemon_id: u8) {
    remove_minor_status_ailments(state, pokemon_id);

    let old_field_pos;
    {
        let pokemon = state.pokemon_by_id_mut(pokemon_id);
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
        let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
        state.add_display_text(format!("Removing {} from field position {:?}.", pokemon_display_text, old_field_pos));
    }

    if let Some(min_pokemon_id) = state.min_pokemon_id {
        let min_pokemon = state.pokemon_by_id_mut(min_pokemon_id);
        if let Some(seeder_id) = min_pokemon.seeded_by {
            if seeder_id == pokemon_id { min_pokemon.seeded_by = None; }
        }
    }
    if let Some(max_pokemon_id) = state.max_pokemon_id {
        let max_pokemon = state.pokemon_by_id_mut(max_pokemon_id);
        if let Some(seeder_id) = max_pokemon.seeded_by {
            if seeder_id == pokemon_id { max_pokemon.seeded_by = None; }
        }
    }

    if state.min_pokemon_id == Some(pokemon_id) {
        state.min_pokemon_id = None;
    } else if state.max_pokemon_id == Some(pokemon_id) {
        state.max_pokemon_id = None;
    } else {
        let pokemon_display_text = format!("{}", state.pokemon_by_id(pokemon_id));
        panic!(format!("ID of {} does not match any ID on the field.", pokemon_display_text));
    }
}

pub fn increment_stat_stage(state: &mut State, pokemon_id: u8, stat_index: StatIndex, requested_amount: i8) {
    let old_stat_stage;
    let new_stat_stage;
    {
        let pokemon = state.pokemon_by_id_mut(pokemon_id);
        old_stat_stage = pokemon.stat_stages[stat_index.as_usize()];
        new_stat_stage = clamp(old_stat_stage + requested_amount, -6, 6);
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

pub fn increment_msa_counter(state: &mut State, pokemon_id: u8) {
    let mut msa_cured = false;
    {
        let pokemon = state.pokemon_by_id_mut(pokemon_id);
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
        let pokemon = state.pokemon_by_id(pokemon_id);
        let species_name = Species::name(pokemon.species);
        let cured_display_text = pokemon.major_status_ailment.display_text_when_cured();
        state.add_display_text(format!("{}{}", species_name, cured_display_text));
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
        return state.battle_end_check();
    }

    let pokemon = state.pokemon_by_id_mut(pokemon_id);
    pokemon.current_hp = min(new_hp as u16, pokemon.max_hp);
    false
}

pub fn increment_move_pp(state: &mut State, pokemon_id: u8, move_index: u8, amount: i8) {
    let move_instance = &mut state.pokemon_by_id_mut(pokemon_id).known_moves[move_index as usize];
    move_instance.pp = clamp(move_instance.pp as i8 + amount, 0, Move::max_pp(move_instance.move_) as i8) as u8;
}

fn remove_minor_status_ailments(state: &mut State, pokemon_id: u8) {
    let pokemon = state.pokemon_by_id_mut(pokemon_id);
    pokemon.confusion_turn_inflicted = None;
    pokemon.confusion_turn_will_cure = None;
    pokemon.is_flinching = false;
    pokemon.seeded_by = None;
    pokemon.is_infatuated = false;
    pokemon.is_cursed = false;
    pokemon.has_nightmare = false;
}
