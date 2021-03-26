use serde::Deserialize;
use std::fmt::Debug;
use std::fs;
use crate::{Type, FieldPosition, game_version};
use crate::battle_ai::move_effects::{MoveEffect, MoveAccuracy};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
pub enum MoveCategory {
    Physical,
    Special,
    Status
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
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

pub type MoveID = u8;

#[derive(Debug, Deserialize)]
pub struct Move {
    name: String,
    #[serde(rename = "type")]
    type_: Type,
    category: MoveCategory,
    accuracy: MoveAccuracy,
    targeting: MoveTargeting,
    max_pp: u8,
    priority_stage: i8,
    sound_based: bool,
    effects: Vec<MoveEffect>
}

impl Move {
    pub fn count() -> MoveID {
        unsafe {
            MOVES.len() as MoveID
        }
    }

    pub fn id_by_name(name: &str) -> Result<MoveID, String> {
        unsafe {
            for (move_id, moves) in MOVES.iter().enumerate() {
                if moves.name.eq_ignore_ascii_case(name) {
                    return Ok(move_id as MoveID);
                }
            }
        }
        Err(format!("invalid move '{}'", name))
    }

    fn by_id(move_id: MoveID) -> &'static Move {
        unsafe {
            &MOVES[move_id as usize]
        }
    }

    pub fn name(move_: MoveID) -> &'static str {
        Move::by_id(move_).name.as_str()
    }

    pub fn category(move_: MoveID) -> MoveCategory {
        let move_ = Move::by_id(move_);
        let category = move_.category;
        if category != MoveCategory::Status && game_version().gen() <= 3 {
            return move_.type_.category();
        }
        category
    }

    pub fn accuracy(move_: MoveID) -> &'static MoveAccuracy {
        &Move::by_id(move_).accuracy
    }

    pub fn targeting(move_: MoveID) -> MoveTargeting {
        Move::by_id(move_).targeting
    }

    pub fn max_pp(move_: MoveID) -> u8 {
        Move::by_id(move_).max_pp
    }

    pub fn priority_stage(move_: MoveID) -> i8 {
        Move::by_id(move_).priority_stage
    }

    pub fn effects(move_: MoveID) -> &'static [MoveEffect] {
        &Move::by_id(move_).effects
    }
}

static mut MOVES: Vec<Move> = Vec::new();

/// # Safety
/// Should be called after the game version has been set from the program input and before the species are initialized.
pub fn initialize_moves() {
    let mut path = String::from("resources/");
    path.push_str(game_version().name());
    path.push_str("/moves.json");
    let moves_json = fs::read_to_string(path.as_str())
        .unwrap_or_else(|_| panic!("Failed to read {}.", path));
    unsafe {
        MOVES = serde_json::from_str(moves_json.as_str())
            .unwrap_or_else(|err| panic!("Error parsing moves.json: {}", err));
    }
}
