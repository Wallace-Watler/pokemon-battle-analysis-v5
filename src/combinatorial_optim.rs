use rand::prelude::StdRng;
use serde::{Deserialize, Serialize};
use crate::battle_ai::pokemon::TeamBuild;
use crate::battle_ai::state;
use rand::Rng;
use rand::distributions::Distribution;
use std::iter;
use statrs::distribution::{Normal, Univariate, StudentsT};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Solution {
    fitness: f64,
    fit_variance: f64,
    num_samples: usize,
    prob_worse_than_best: f64,
    team_build: TeamBuild
}

impl Solution {
    fn new(rng: &mut StdRng) -> Solution {
        Solution {
            fitness: -1.0,
            fit_variance: 0.0,
            num_samples: 0,
            prob_worse_than_best: 0.0,
            team_build: TeamBuild::new(rng)
        }
    }

    /// Update this solution with a new fitness sample.
    fn update(&mut self, fitness_sample: f64) {
        match self.num_samples {
            0 => self.fitness = fitness_sample,
            1 => {
                let old_fitness = self.fitness;
                self.fitness = (old_fitness * self.num_samples as f64 + fitness_sample) / (self.num_samples + 1) as f64;
                self.fit_variance = (fitness_sample - old_fitness).powf(2.0);
            },
            _ => {
                let old_fitness = self.fitness;
                let old_variance = self.fit_variance;
                self.fitness = (old_fitness * self.num_samples as f64 + fitness_sample) / (self.num_samples + 1) as f64;
                self.fit_variance = ((self.num_samples - 1) as f64 * old_variance + (fitness_sample - self.fitness) * (fitness_sample - old_fitness)) / self.num_samples as f64;
            }
        }
        self.num_samples += 1;
    }

    /// Returns the probability that this solution performs worse than `other`.
    fn prob_worse_than(&self, other: &Solution, rng: &mut StdRng) -> f64 {
        if almost::zero(self.fit_variance) && almost::zero(other.fit_variance) {
            return if self.fitness > other.fitness {
                0.0
            } else if self.fitness < other.fitness {
                1.0
            } else {
                0.5
            }
        }

        if self.num_samples > 30 && other.num_samples > 30 {
            Normal::new(self.fitness - other.fitness, (self.fit_variance + other.fit_variance).sqrt()).unwrap().cdf(0.0)
        } else {
            let t_dist_1 = StudentsT::new(self.fitness, self.fit_variance.sqrt(), (self.num_samples - 1) as f64).unwrap();
            let t_dist_2 = StudentsT::new(other.fitness, other.fit_variance.sqrt(), (other.num_samples - 1) as f64).unwrap();

            const MONTE_CARLO_NUM: usize = 100000;
            let mut count = 0;
            for _ in 0..MONTE_CARLO_NUM {
                if t_dist_1.sample(rng) - t_dist_2.sample(rng) < 0.0 {
                    count += 1;
                }
            }

            count as f64 / MONTE_CARLO_NUM as f64
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Solver {
    fitness_func_evals: usize,
    maximizer_meta: Vec<Solution>,
    minimizer_meta: Vec<Solution>
}

impl Solver {
    pub fn new(rng: &mut StdRng) -> Solver {
        Solver {
            fitness_func_evals: 0,
            maximizer_meta: iter::repeat_with(|| Solution::new(rng)).take(100).collect(),
            minimizer_meta: iter::repeat_with(|| Solution::new(rng)).take(100).collect()
        }
    }

    pub fn maximizer_meta(&self) -> &[Solution] {
        &self.maximizer_meta
    }

    pub fn minimizer_meta(&self) -> &[Solution] {
        &self.minimizer_meta
    }

    pub fn do_iter(&mut self, rng: &mut StdRng) {
        // Evaluate solutions in one meta against those in the other, updating their fitnesses.
        let interaction_chance = 1.0 / ((self.maximizer_meta.len() * self.minimizer_meta.len()) as f64).sqrt();
        for max_sol in self.maximizer_meta.iter_mut() {
            for min_sol in self.minimizer_meta.iter_mut() {
                if rng.gen_bool(interaction_chance) {
                    let fitness_sample = state::run_battle(&min_sol.team_build, &max_sol.team_build, rng);
                    self.fitness_func_evals += 1;
                    max_sol.update(fitness_sample);
                    min_sol.update(-fitness_sample);
                }
            }
        }

        Solver::update_meta(&mut self.maximizer_meta, rng);
        Solver::update_meta(&mut self.minimizer_meta, rng);
    }

    fn update_meta(meta: &mut Vec<Solution>, rng: &mut StdRng) {
        let num_sols = meta.len();

        // Each solution may create a child according to its probability of performing better than the best solution.
        meta.sort_unstable_by(|sol1, sol2| sol2.fitness.partial_cmp(&sol1.fitness).unwrap());
        for i in 0..num_sols {
            if meta[i].num_samples < 2 || meta[0].num_samples < 2 {
                meta[i].prob_worse_than_best = 0.0;
            } else {
                meta[i].prob_worse_than_best = meta[i].prob_worse_than(&meta[0], rng);
                if rng.gen_bool((1.0 - meta[i].prob_worse_than_best) / num_sols as f64) {
                    let child = meta[i].team_build.mutated_child(rng);

                    // Check if there is already a solution stored with the chosen team.
                    // If there isn't, create a new solution and add it to the meta.
                    if meta.iter().find(|sol| sol.team_build == child).is_none() {
                        meta.push(Solution {
                            fitness: -1.0,
                            fit_variance: 0.0,
                            num_samples: 0,
                            prob_worse_than_best: 0.0,
                            team_build: child
                        });
                    }
                }
            }
        }

        // Remove solutions that are not likely to be better than the best solution.
        let p_cutoff = meta[0].fitness / 4.0 + 0.75;
        meta.retain(|sol| sol.prob_worse_than_best < p_cutoff);
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
