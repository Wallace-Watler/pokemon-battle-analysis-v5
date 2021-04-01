use rand::prelude::{StdRng, SliceRandom};
use rand::Rng;
use serde::export::TryFrom;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::fmt::{Display, Error, Formatter};
use crate::{StatIndex, MajorStatusAilment, Ability, game_version, FieldPosition, Weather, Type, Gender, Nature, AbilityID, choose_weighted_index, Counter};
use crate::species::{SpeciesID, Species};
use crate::battle_ai::move_effects::Action;
use crate::move_::{MoveID, Move};
use crate::battle_ai::state::State;

#[derive(Clone, Debug)]
/// Assumed to be level 100.
pub struct Pokemon {
    pub species: SpeciesID,
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
    pub msa_counter: Counter<u16>,
    /// Only used in gen 3; otherwise it's always 0.
    snore_sleep_talk_counter: u16,

    // Minor status ailments
    confusion_counter: Counter<u16>,
    is_flinching: bool,
    /// Position of the Pokemon that seeded this Pokemon.
    pub seeded_by: Option<FieldPosition>,
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

    pub fn known_move(&self, move_index: usize) -> &MoveInstance {
        &self.known_moves[move_index]
    }

    pub fn known_moves(&self) -> &[MoveInstance] {
        &self.known_moves
    }

    pub fn is_type(&self, type_: Type) -> bool {
        self.first_type == type_ || self.second_type == type_
    }

    pub fn can_choose_move(&self, move_index: usize) -> bool {
        let move_instance = &self.known_moves[move_index];
        self.current_hp > 0 && self.field_position.is_some() && move_instance.pp > 0 && !move_instance.disabled
    }
}

impl From<&PokemonBuild> for Pokemon {
    fn from(pb: &PokemonBuild) -> Self {
        Pokemon {
            species: pb.species,
            first_type: Species::type1(pb.species),
            second_type: Species::type2(pb.species),
            gender: pb.gender,
            nature: pb.nature,
            ability: pb.ability,
            ivs: pb.ivs,
            evs: pb.ivs,
            max_hp: pb.max_hp(),
            current_hp: pb.max_hp(),
            stat_stages: [0; 8],
            major_status_ailment: MajorStatusAilment::Okay,
            msa_counter: Counter {
                value: 0,
                target: None
            },
            snore_sleep_talk_counter: 0,
            confusion_counter: Counter::new(None),
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
#[derive(Clone, Debug, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "PokemonBuildSerde", into = "PokemonBuildSerde")]
pub struct PokemonBuild {
    pub species: SpeciesID,
    pub gender: Gender,
    pub nature: Nature,
    pub ability: AbilityID,
    pub ivs: [u8; 6],
    /// EVs are assigned as 127 groups of 4 points each, totaling 508 points. This is two less than
    /// the actual limit of 510, but the extra two points are wasted anyways due to how stats are
    /// calculated. Furthermore, restricting the EVs to multiples of 4 reduces the number of
    /// possible team builds by a factor of ~778 quadrillion.
    pub evs: [u8; 6],
    /// Contains 1-4 moves.
    pub moves: Vec<MoveID>
}

impl PokemonBuild {
    pub const fn num_vars() -> usize {
        20
    }

    pub fn new(rng: &mut StdRng) -> PokemonBuild {
        let mut evs = [0; 6];
        let mut ev_sum = 0;
        while ev_sum < 508 {
            let i = rng.gen_range(0, 6);
            if evs[i] < 252 {
                evs[i] += 4;
                ev_sum += 4;
            }
        }

        let species = Species::random_species(rng);
        PokemonBuild {
            species,
            gender: Species::random_gender(species, rng),
            nature: Nature::random_nature(rng),
            ability: Species::random_ability(species, rng),
            ivs: [
                rng.gen_range(0, 32),
                rng.gen_range(0, 32),
                rng.gen_range(0, 32),
                rng.gen_range(0, 32),
                rng.gen_range(0, 32),
                rng.gen_range(0, 32)
            ],
            evs,
            moves: Species::random_move_set(species, rng),
        }
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

    pub fn moves(&self) -> &[MoveID] {
        &self.moves
    }

    pub fn max_hp(&self) -> u16 {
        2 * Species::base_stat(self.species, StatIndex::Hp) as u16 + self.ivs[StatIndex::Hp.as_usize()] as u16 + self.evs[StatIndex::Hp.as_usize()] as u16 / 4 + 110
    }
}

impl PartialEq for PokemonBuild {
    fn eq(&self, other: &Self) -> bool {
        for move_ in &self.moves {
            if !other.moves.contains(move_) {
                return false;
            }
        }
        self.species == other.species
            && self.gender == other.gender
            && self.nature == other.nature
            && self.ability == other.ability
            && self.ivs == other.ivs
            && self.evs == other.evs
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
            moves: vec![
                Move::id_by_name(pb_serde.move1)?,
                Move::id_by_name(pb_serde.move2)?,
                Move::id_by_name(pb_serde.move3)?,
                Move::id_by_name(pb_serde.move4)?
            ]
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

#[derive(Clone, Debug, Eq, Deserialize, Serialize)]
pub struct TeamBuild {
    pub members: [PokemonBuild; 6]
}

impl TeamBuild {
    pub const fn num_vars() -> usize {
        PokemonBuild::num_vars() * 6
    }

    pub fn new(rng: &mut StdRng) -> TeamBuild {
        let mut non_duplicates = Vec::new();
        let mut new_member = || -> PokemonBuild {
            let mut result = PokemonBuild::new(rng);
            while non_duplicates.contains(&result.species) {
                result = PokemonBuild::new(rng);
            }
            if !Species::allow_duplicates(result.species) {
                non_duplicates.push(result.species);
            }
            result
        };

        TeamBuild {
            members: [
                new_member(),
                new_member(),
                new_member(),
                new_member(),
                new_member(),
                new_member()
            ]
        }
    }

    pub fn mutated_child(&self, rng: &mut StdRng) -> TeamBuild {
        let member_num = rng.gen_range(0, 6);
        let build_to_mutate = &self.members[member_num];

        // Each variable's mutation rate is proportional to the number of other choices for that variable.
        let mutation_rates = [
            (
                Species::count() as usize - 1
                - self.members.iter().filter(|b| !Species::allow_duplicates(b.species)).count()
                + if !Species::allow_duplicates(build_to_mutate.species) { 1 } else { 0 }
            ) as f64,
            if Species::has_male_and_female(build_to_mutate.species) { 1.0 } else { 0.0 },
            24.0,
            (Species::abilities(build_to_mutate.species).len() - 1) as f64,
            31.0 * 6.0,
            30.0 + 30.0,
            {
                let p = Species::move_pool(build_to_mutate.species).len();
                let m = build_to_mutate.moves.len();
                ((p - m) * m) as f64
            }
        ];

        let mut child = self.clone();
        let mut child_build = &mut child.members[member_num];
        match choose_weighted_index(&mutation_rates, rng) {
            0 => {
                while build_to_mutate.species == child_build.species || (!Species::allow_duplicates(child_build.species) && self.members.iter().any(|b| b.species == child_build.species)) {
                    child_build.species = Species::random_species(rng);
                }
                child_build.gender = Species::random_gender(child_build.species, rng);
                child_build.ability = Species::random_ability(child_build.species, rng);
                child_build.moves = Species::random_move_set(child_build.species, rng);
            },
            1 => child_build.gender = child_build.gender.opposite(),
            2 => {
                let old_nature = child_build.nature;
                while child_build.nature == old_nature {
                    child_build.nature = Nature::random_nature(rng);
                }
            },
            3 => {
                let old_ability = child_build.ability;
                while child_build.ability == old_ability {
                    child_build.ability = Species::random_ability(child_build.species, rng);
                }
            },
            4 => {
                let i = rng.gen_range(0, 6);
                let old_iv = child_build.ivs[i];
                while child_build.ivs[i] == old_iv {
                    child_build.ivs[i] = rng.gen_range(0, 32);
                }
            },
            5 => {
                if rng.gen_bool(0.5) {
                    let i = rng.gen_range(0, 6);
                    let mut j = rng.gen_range(0, 6);
                    while j == i {
                        j = rng.gen_range(0, 6);
                    }
                    child_build.evs.swap(i, j);
                } else {
                    let mut from = rng.gen_range(0, 6);
                    while child_build.evs[from] < 4 {
                        from = rng.gen_range(0, 6);
                    }
                    let mut to = rng.gen_range(0, 6);
                    while to == from || child_build.evs[to] >= 252 {
                        to = rng.gen_range(0, 6);
                    }
                    child_build.evs[from] -= 4;
                    child_build.evs[to] += 4;
                }
            },
            _ => {
                let move_pool = Species::move_pool(child_build.species);
                let mut new_move = move_pool.choose(rng).unwrap();
                while child_build.moves.contains(new_move) {
                    new_move = move_pool.choose(rng).unwrap();
                }
                *child_build.moves.choose_mut(rng).unwrap() = *new_move;
            }
        }
        child
    }
}

impl PartialEq for TeamBuild {
    fn eq(&self, other: &Self) -> bool {
        // Two teams are equal if their party leaders are equal and the rest of their team members
        // are found on both teams, in any order. The party leader is separate since they are always
        // the first Pokemon sent out. The rest of the team can be freely switched in and out of
        // battle.
        for team_member in self.members[1..6].iter() {
            if !other.members[1..6].contains(team_member) {
                return false;
            }
        }
        self.members[0] == other.members[0]
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
pub fn poison(state: &mut State, pokemon_id: u8, toxic: bool, corrosion: bool) -> bool {
    let pokemon = state.pokemon_by_id_mut(pokemon_id);

    if !corrosion && (pokemon.is_type(Type::Poison) || pokemon.is_type(Type::Steel)) {
        if cfg!(feature = "print-battle") {
            let species_name = Species::name(state.pokemon_by_id(pokemon_id).species);
            state.add_display_text(format!("It doesn't affect the opponent's {} ...", species_name));
        }
        return false;
    }

    if pokemon.major_status_ailment() == MajorStatusAilment::Okay {
        pokemon.major_status_ailment = if toxic { MajorStatusAilment::BadlyPoisoned } else { MajorStatusAilment::Poisoned };
        pokemon.msa_counter.clear();
        if cfg!(feature = "print-battle") {
            let species_name = Species::name(state.pokemon_by_id(pokemon_id).species);
            state.add_display_text(format!("{}{}", species_name, if toxic { MajorStatusAilment::BadlyPoisoned.display_text_when_applied() } else { MajorStatusAilment::Poisoned.display_text_when_applied() }));
        }
        return true;
    }

    if cfg!(feature = "print-battle") {
        state.add_display_text(String::from("But it failed!"));
    }
    false
}

/// Returns whether the Pokemon fell asleep.
pub fn put_to_sleep(state: &mut State, pokemon_id: u8, rng: &mut StdRng) -> bool {
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
        return true;
    }

    if cfg!(feature = "print-battle") {
        state.add_display_text(String::from("But it failed!"));
    }
    false
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
