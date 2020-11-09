use json::JsonValue;
use std::fmt::{Debug, Error, Formatter};
use std::fs;
use std::process;
use crate::{Type, StatIndex, FieldPosition, game_version};
use crate::battle_ai::move_effects::MoveEffect;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MoveCategory {
    Physical,
    Special,
    Status
}

impl MoveCategory {
    fn by_name(name: &str) -> Result<MoveCategory, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "physical" => Ok(MoveCategory::Physical),
            "special"  => Ok(MoveCategory::Special),
            "status"   => Ok(MoveCategory::Status),
            _ => Err(format!("Invalid move category '{}'", name))
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
    fn by_name(name: &str) -> Result<MoveTargeting, String> {
        let n = name.to_ascii_lowercase();
        match n.as_str() {
            "randomopponent"         => Ok(MoveTargeting::RandomOpponent),
            "singleadjacentally"     => Ok(MoveTargeting::SingleAdjacentAlly),
            "singleadjacentopponent" => Ok(MoveTargeting::SingleAdjacentOpponent),
            "singleadjacentpokemon"  => Ok(MoveTargeting::SingleAdjacentPokemon),
            "singlepokemon"          => Ok(MoveTargeting::SinglePokemon),
            "user"                   => Ok(MoveTargeting::User),
            "useroradjacentally"     => Ok(MoveTargeting::UserOrAdjacentAlly),
            "userandallallies"       => Ok(MoveTargeting::UserAndAllAllies),
            "alladjacentopponents"   => Ok(MoveTargeting::AllAdjacentOpponents),
            "alladjacentpokemon"     => Ok(MoveTargeting::AllAdjacentPokemon),
            "allallies"              => Ok(MoveTargeting::AllAllies),
            "allopponents"           => Ok(MoveTargeting::AllOpponents),
            "allpokemon"             => Ok(MoveTargeting::AllPokemon),
            _ => Err(format!("Invalid move targeting '{}'", name))
        }
    }

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

pub struct Move {
    name: String,
    type_: Type,
    category: MoveCategory,
    /// An accuracy of 0 means this move ignores accuracy checks and will always hit.
    accuracy: u8,
    targeting: MoveTargeting,
    max_pp: u8,
    priority_stage: i8,
    sound_based: bool,
    effects: Vec<MoveEffect>
}

impl Move {
    pub fn id_by_name(name: &str) -> Result<MoveID, String> {
        unsafe {
            for (move_id, moves) in MOVES.iter().enumerate() {
                if moves.name.eq_ignore_ascii_case(name) {
                    return Ok(move_id as MoveID);
                }
            }
        }
        Err(format!("Invalid move '{}'", name))
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

    pub fn accuracy(move_: MoveID) -> u8 {
        Move::by_id(move_).accuracy
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

static mut MOVES: Vec<Move> = Vec::new();

/// # Safety
/// Should be called after the game version has been set from the program input and before the species are initialized.
pub fn initialize_moves() {
    let mut path = String::from("resources/");
    path.push_str(game_version().name());
    path.push_str("/moves.json");
    let moves_json = fs::read_to_string(path.as_str()).unwrap_or_else(|_| panic!("Failed to read {}.", path));

    match json::parse(moves_json.as_str()) {
        json::Result::Ok(parsed) => {
            match parsed {
                JsonValue::Array(array) => {
                    let extract_string = |json_value: &mut JsonValue, key: &str| -> String {
                        json_value.remove(key).as_str()
                            .unwrap_or_else(|| panic!("Invalid moves.json: member\n{}\ndoes not have a valid string field '{}'", json_value.pretty(4), key))
                            .to_owned()
                    };
                    let extract_type = |json_value: &mut JsonValue, key: &str| -> Type {
                        let string = extract_string(json_value, key);
                        Type::by_name(string.as_str())
                            .unwrap_or_else(|_| panic!("Invalid moves.json: '{}' in object\n{}\nis not a valid {}", string, json_value.pretty(4), key))
                    };
                    let extract_type_def = |json_value: &mut JsonValue, key: &str, default: Type| -> Type {
                        if !json_value.has_key(key) { return default; }
                        let string = extract_string(json_value, key);
                        Type::by_name(string.as_str()).unwrap_or_else(|_| panic!("Invalid moves.json: '{}' in member\n{}\nis not a valid {}", string, json_value.pretty(4), key))
                    };
                    let extract_category = |json_value: &mut JsonValue, key: &str| -> MoveCategory {
                        let string = extract_string(json_value, key);
                        MoveCategory::by_name(string.as_str())
                            .unwrap_or_else(|_| panic!("Invalid moves.json: '{}' in object\n{}\nis not a valid {}", string, json_value.pretty(4), key))
                    };
                    let extract_targeting = |json_value: &mut JsonValue, key: &str| -> MoveTargeting {
                        let string = extract_string(json_value, key);
                        MoveTargeting::by_name(string.as_str())
                            .unwrap_or_else(|_| panic!("Invalid moves.json: '{}' in object\n{}\nis not a valid {}", string, json_value.pretty(4), key))
                    };
                    let extract_u8 = |json_value: &mut JsonValue, key: &str| -> u8 {
                        json_value.remove(key).as_u8()
                            .unwrap_or_else(|| panic!("Invalid moves.json: member\n{}\ndoes not have a valid u8 field '{}'", json_value.pretty(4), key))
                    };
                    let extract_u8_def = |json_value: &mut JsonValue, key: &str, default: u8| -> u8 {
                        if !json_value.has_key(key) { return default; }
                        json_value.remove(key).as_u8()
                            .unwrap_or_else(|| panic!("Invalid moves.json: member\n{}\nhas an invalid u8 field '{}'", json_value.pretty(4), key))
                    };
                    let extract_i8 = |json_value: &mut JsonValue, key: &str| -> i8 {
                        json_value.remove(key).as_i8()
                            .unwrap_or_else(|| panic!("Invalid moves.json: member\n{}\ndoes not have a valid i8 field '{}'", json_value.pretty(4), key))
                    };
                    let extract_bool = |json_value: &mut JsonValue, key: &str| -> bool {
                        json_value.remove(key).as_bool()
                            .unwrap_or_else(|| panic!("Invalid moves.json: member\n{}\ndoes not have a valid boolean field '{}'", json_value.pretty(4), key))
                    };

                    for mut json_move in array {
                        let type_ = extract_type(&mut json_move, "type");
                        let mut effects = Vec::new();

                        for mut json_effect in json_move.remove("effects").members_mut() {
                            let effect;
                            let name = extract_string(&mut json_effect, "name");
                            match name.as_str() {
                                "StdDamage" => {
                                    effect = MoveEffect::StdDamage(
                                        extract_type_def(&mut json_effect, "damage_type", type_),
                                        extract_u8(&mut json_effect, "power"),
                                        extract_u8_def(&mut json_effect, "critical_hit_stage_bonus", 0),
                                        extract_u8_def(&mut json_effect, "recoil_divisor", 0)
                                    );
                                },
                                "IncTargetStatStage" => {
                                    effect = MoveEffect::IncTargetStatStage(
                                        StatIndex::by_name(extract_string(&mut json_effect, "stat_index").as_str())
                                            .unwrap_or_else(|_| panic!("Invalid moves.json: 'stat_index' in object\n{}\nis not a valid stat index", json_effect.pretty(4))),
                                        extract_i8(&mut json_effect, "amount")
                                    );
                                },
                                "LeechSeed" => effect = MoveEffect::LeechSeed,
                                "Struggle" => effect = MoveEffect::Struggle,
                                _ => panic!("Invalid moves.json: '{}' in move effect\n{}\nis not a valid move effect", name, json_effect.pretty(4))
                            }
                            effects.push(effect);
                        }

                        let category = extract_category(&mut json_move, "category");
                        unsafe {
                            MOVES.push(Move {
                                name: extract_string(&mut json_move, "name").to_owned(),
                                type_,
                                category: if game_version().gen() <= 3 && category != MoveCategory::Status { type_.category() } else { category },
                                accuracy: extract_u8(&mut json_move, "accuracy"),
                                targeting: extract_targeting(&mut json_move, "targeting"),
                                max_pp: extract_u8(&mut json_move, "max_pp"),
                                priority_stage: extract_i8(&mut json_move, "priority_stage"),
                                sound_based: extract_bool(&mut json_move, "sound_based"),
                                effects
                            });
                        }
                    }
                },
                _ => panic!("Invalid moves.json: not an array of objects")
            }
        },
        json::Result::Err(error) => {
            println!("{}", error);
            process::exit(1);
        }
    }
}
