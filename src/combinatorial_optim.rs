use rand::prelude::StdRng;
use rand_distr::{StudentT, Distribution};
use serde::{Deserialize, Serialize};
use serde::export::TryFrom;
use std::cmp::{Ordering, min};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::Hash;
use std::iter::FromIterator;
use crate::battle_ai::pokemon::{TeamBuild, PokemonBuild};
use crate::battle_ai::state;
use crate::{Gender, Nature, Ability, AbilityID, choose_weighted_index};
use crate::species::{Species, SpeciesID};
use crate::move_::{Move, MoveID};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Solution {
    fitness: f64,
    fit_variance: f64,
    num_samples: usize,
    team_build: TeamBuild
}

impl Solution {
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

    fn is_worse_than(&self, other: &Solution, p_cutoff: f64, rng: &mut StdRng) -> bool {
        if self.num_samples < 2 || other.num_samples < 2 { return false; }

        let t_dist_1 = StudentT::new((self.num_samples - 1) as f64).unwrap();
        let t_dist_2 = StudentT::new((other.num_samples - 1) as f64).unwrap();

        const MONTE_CARLO_NUM: usize = 100000;
        let mut count = 0;
        for _ in 0..MONTE_CARLO_NUM {
            let t_sample_1 = t_dist_1.sample(rng) * (self.fit_variance / self.num_samples as f64).sqrt() + self.fitness;
            let t_sample_2 = t_dist_2.sample(rng) * (other.fit_variance / other.num_samples as f64).sqrt() + other.fitness;
            if t_sample_1 - t_sample_2 < 0.0 {
                count += 1;
                if count as f64 >= p_cutoff * MONTE_CARLO_NUM as f64 {
                    return true;
                }
            }
        }

        false
    }
}

/// Data pertaining to a variable of the solution space.
#[derive(Clone)]
struct VariableData {
    /// Used to randomly select a choice for this variable.
    weights: Vec<f64>,
    /// The number of times each choice of this variable has been explored.
    counts: Vec<usize>
}

impl VariableData {
    fn add_fitness_sample(&mut self, var_choice: usize, fitness_sample: f64) {
        let old_count = *self.counts.get(var_choice).unwrap();
        let old_weight = *self.weights.get(var_choice).unwrap();
        let new_count = old_count + 1;
        *self.weights.get_mut(var_choice).unwrap() = (old_weight * old_count as f64 + fitness_sample) / new_count as f64;
        *self.counts.get_mut(var_choice).unwrap() = new_count;
    }
}

#[derive(Deserialize, Serialize)]
struct VariableDataSerde<T: Eq + Hash + Ord> {
    weights: BTreeMap<T, f64>,
    counts: BTreeMap<T, usize>
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(try_from = "PokeVarDataSerde", into = "PokeVarDataSerde")]
struct PokeVarData {
    species: VariableData,
    gender: VariableData,
    nature: VariableData,
    ability: VariableData,
    ivs: [VariableData; 6],
    evs: [VariableData; 6],
    // All move data is combined since their order does not matter.
    moves: VariableData
}

impl PokeVarData {
    fn new() -> PokeVarData {
        let iv_var_data = VariableData {
            weights: vec![1.0; 32],
            counts: vec![1; 32]
        };
        let ev_var_data = VariableData {
            weights: vec![1.0; 253],
            counts: vec![1; 253]
        };
        PokeVarData {
            species: VariableData {
                weights: vec![1.0; Species::count() as usize],
                counts: vec![1; Species::count() as usize]
            },
            gender: VariableData {
                weights: vec![1.0; Gender::count() as usize],
                counts: vec![1; Gender::count() as usize]
            },
            nature: VariableData {
                weights: vec![1.0; Nature::count() as usize],
                counts: vec![1; Nature::count() as usize]
            },
            ability: VariableData {
                weights: vec![1.0; Ability::count() as usize],
                counts: vec![1; Ability::count() as usize]
            },
            ivs: [iv_var_data.clone(), iv_var_data.clone(), iv_var_data.clone(), iv_var_data.clone(), iv_var_data.clone(), iv_var_data],
            evs: [ev_var_data.clone(), ev_var_data.clone(), ev_var_data.clone(), ev_var_data.clone(), ev_var_data.clone(), ev_var_data],
            moves: VariableData {
                weights: vec![1.0; Move::count() as usize],
                counts: vec![1; Move::count() as usize]
            }
        }
    }
}

impl TryFrom<PokeVarDataSerde<'_>> for PokeVarData {
    type Error = String;

    fn try_from(pvd_serde: PokeVarDataSerde<'_>) -> Result<Self, Self::Error> {
        let mut poke_var_data = PokeVarData::new();
        for (&species, &weight) in pvd_serde.species.weights.iter() { poke_var_data.species.weights[Species::id_by_name(species)? as usize] = weight; }
        for (&species, &count) in pvd_serde.species.counts.iter() { poke_var_data.species.counts[Species::id_by_name(species)? as usize] = count; }
        for (&gender, &weight) in pvd_serde.gender.weights.iter() { poke_var_data.gender.weights[gender.id() as usize] = weight; }
        for (&gender, &count) in pvd_serde.gender.counts.iter() { poke_var_data.gender.counts[gender.id() as usize] = count; }
        for (&nature, &weight) in pvd_serde.nature.weights.iter() { poke_var_data.nature.weights[nature.id() as usize] = weight; }
        for (&nature, &count) in pvd_serde.nature.counts.iter() { poke_var_data.nature.counts[nature.id() as usize] = count; }
        for (&ability, &weight) in pvd_serde.ability.weights.iter() { poke_var_data.ability.weights[Ability::id_by_name(ability)? as usize] = weight; }
        for (&ability, &count) in pvd_serde.ability.counts.iter() { poke_var_data.ability.counts[Ability::id_by_name(ability)? as usize] = count; }
        (0..6).into_iter().for_each(|stat| {
            for (&iv, &weight) in pvd_serde.ivs[stat].weights.iter() { poke_var_data.ivs[stat].weights[iv as usize] = weight; }
            for (&iv, &count) in pvd_serde.ivs[stat].counts.iter() { poke_var_data.ivs[stat].counts[iv as usize] = count; }
            for (&ev, &weight) in pvd_serde.evs[stat].weights.iter() { poke_var_data.evs[stat].weights[ev as usize] = weight; }
            for (&ev, &count) in pvd_serde.evs[stat].counts.iter() { poke_var_data.evs[stat].counts[ev as usize] = count; }
        });
        for (&move_, &weight) in pvd_serde.moves.weights.iter() { poke_var_data.moves.weights[Move::id_by_name(move_)? as usize] = weight; }
        for (&move_, &count) in pvd_serde.moves.counts.iter() { poke_var_data.moves.counts[Move::id_by_name(move_)? as usize] = count; }
        Ok(poke_var_data)
    }
}

#[derive(Deserialize, Serialize)]
struct PokeVarDataSerde<'d> {
    #[serde(borrow)]
    species: VariableDataSerde<&'d str>,
    gender: VariableDataSerde<Gender>,
    nature: VariableDataSerde<Nature>,
    #[serde(borrow)]
    ability: VariableDataSerde<&'d str>,
    ivs: Vec<VariableDataSerde<u8>>,
    evs: Vec<VariableDataSerde<u8>>,
    #[serde(borrow)]
    moves: VariableDataSerde<&'d str>
}

impl From<PokeVarData> for PokeVarDataSerde<'_> {
    fn from(poke_var_data: PokeVarData) -> Self {
        PokeVarDataSerde {
            species: VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.species.weights.iter().enumerate().map(|(id, &weight)| (Species::name(id as SpeciesID), weight))),
                counts: BTreeMap::from_iter(poke_var_data.species.counts.iter().enumerate().map(|(id, &count)| (Species::name(id as SpeciesID), count)))
            },
            gender: VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.gender.weights.iter().enumerate().map(|(id, &weight)| (Gender::by_id(id as u8), weight))),
                counts: BTreeMap::from_iter(poke_var_data.gender.counts.iter().enumerate().map(|(id, &count)| (Gender::by_id(id as u8), count)))
            },
            nature: VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.nature.weights.iter().enumerate().map(|(id, &weight)| (Nature::by_id(id as u8), weight))),
                counts: BTreeMap::from_iter(poke_var_data.nature.counts.iter().enumerate().map(|(id, &count)| (Nature::by_id(id as u8), count)))
            },
            ability: VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.ability.weights.iter().enumerate().map(|(id, &weight)| (Ability::name(id as AbilityID), weight))),
                counts: BTreeMap::from_iter(poke_var_data.ability.counts.iter().enumerate().map(|(id, &count)| (Ability::name(id as AbilityID), count)))
            },
            ivs: (0..6).into_iter().map(|stat| VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.ivs[stat].weights.iter().enumerate().map(|(id, &weight)| (id as u8, weight))),
                counts: BTreeMap::from_iter(poke_var_data.ivs[stat].counts.iter().enumerate().map(|(id, &count)| (id as u8, count)))
            }).collect(),
            evs: (0..6).into_iter().map(|stat| VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.evs[stat].weights.iter().enumerate().map(|(id, &weight)| (id as u8, weight))),
                counts: BTreeMap::from_iter(poke_var_data.evs[stat].counts.iter().enumerate().map(|(id, &count)| (id as u8, count)))
            }).collect(),
            moves: VariableDataSerde {
                weights: BTreeMap::from_iter(poke_var_data.moves.weights.iter().enumerate().map(|(id, &weight)| (Move::name(id as MoveID), weight))),
                counts: BTreeMap::from_iter(poke_var_data.moves.counts.iter().enumerate().map(|(id, &count)| (Move::name(id as MoveID), count)))
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Solver {
    fitness_func_evals: usize,
    best_solutions: Vec<Solution>,
    party_lead_data: PokeVarData,
    // Data on team members 2-6 are combined since their order does not matter.
    remaining_team_data: PokeVarData
}

impl Solver {
    pub fn new() -> Solver {
        Solver {
            fitness_func_evals: 0,
            best_solutions: Vec::new(),
            party_lead_data: PokeVarData::new(),
            remaining_team_data: PokeVarData::new()
        }
    }

    pub fn best_solutions(&self) -> &[Solution] {
        &self.best_solutions
    }

    pub fn do_iter(&mut self, p_cutoff: f64, refinement_iters: usize, rng: &mut StdRng) {
        let chosen_team = self.random_team_build(rng);
        let fitness_sample = self.fitness_func(&chosen_team, rng);

        // Check if there is already a solution stored with the chosen team.
        // If there is, update the solution corresponding to the chosen team with the new fitness sample.
        let mut chosen_team_has_solution = false;
        if let Some(bs_with_same_team) = self.best_solutions.iter_mut().find(|bs| bs.team_build == chosen_team) {
            bs_with_same_team.update(fitness_sample);
            chosen_team_has_solution = true;
        }

        // If there isn't, create a new solution and add it to the list.
        if !chosen_team_has_solution {
            self.best_solutions.push(Solution {
                fitness: fitness_sample,
                fit_variance: f64::INFINITY,
                num_samples: 1,
                team_build: chosen_team
            });
        }

        self.update_best_solutions(p_cutoff, refinement_iters, rng);
    }

    /// Takes new samples for the known solutions until they have "enough". Does not explore new solutions.
    pub fn refine_best_solutions(&mut self, p_cutoff: f64, refinement_iters: usize, rng: &mut StdRng) {
        while self.best_solutions.iter().any(|bs| bs.num_samples < refinement_iters) {
            self.update_best_solutions(p_cutoff, refinement_iters, rng);
        }
    }

    /// Update the solutions that haven't had enough refinement yet, then remove any that are not likely to be better than the best solution.
    fn update_best_solutions(&mut self, p_cutoff: f64, refinement_iters: usize, rng: &mut StdRng) {
        let mut fitness_samples = Vec::new();
        let best_solutions = self.best_solutions.clone();
        best_solutions.iter().enumerate()
            .filter(|(_, bs)| bs.num_samples < refinement_iters)
            .for_each(|(i, bs)| {
                fitness_samples.push((i, self.fitness_func(&bs.team_build, rng)));
            });
        fitness_samples.iter().for_each(|(i, fs)| {
            self.best_solutions[*i].update(*fs);
        });

        self.best_solutions.sort_by(|bs1, bs2| {
            if bs1.fitness > bs2.fitness { return Ordering::Less }
            if bs1.fitness < bs2.fitness { return Ordering::Greater }
            Ordering::Equal
        });
        if let Some(best_of_the_best) = self.best_solutions.get(0).cloned() {
            self.best_solutions.retain(|bs| !bs.is_worse_than(&best_of_the_best, p_cutoff, rng));
        }
    }

    fn fitness_func(&mut self, maximizer: &TeamBuild, rng: &mut StdRng) -> f64 {
        self.fitness_func_evals += 1;
        // TODO: Let minimizer be specified in program input
        let fitness_sample = (state::run_battle(&TeamBuild::new(rng), maximizer, rng) + 1.0) / 2.0;

        let update_var_data_with_new_sample = |poke_var_data: &mut PokeVarData, pokemon_build: &PokemonBuild| {
            poke_var_data.species.add_fitness_sample(pokemon_build.species() as usize, fitness_sample);
            poke_var_data.gender.add_fitness_sample(pokemon_build.gender().id() as usize, fitness_sample);
            poke_var_data.nature.add_fitness_sample(pokemon_build.nature().id() as usize, fitness_sample);
            poke_var_data.ability.add_fitness_sample(pokemon_build.ability() as usize, fitness_sample);
            for stat in 0..6 {
                poke_var_data.ivs[stat].add_fitness_sample(pokemon_build.ivs()[stat] as usize, fitness_sample);
                poke_var_data.evs[stat].add_fitness_sample(pokemon_build.evs()[stat] as usize, fitness_sample);
            }
            for &move_ in pokemon_build.moves() {
                poke_var_data.moves.add_fitness_sample(move_ as usize, fitness_sample);
            }
        };

        update_var_data_with_new_sample(&mut self.party_lead_data, &maximizer.party_leader);
        for team_member in &maximizer.remaining_team {
            update_var_data_with_new_sample(&mut self.remaining_team_data, team_member);
        }

        fitness_sample
    }

    /// Chooses a random value for each variable, weighted appropriately, to create a new team.
    fn random_team_build(&self, rng: &mut StdRng) -> TeamBuild {
        let mut used_species = HashSet::new();

        let cwip = |weights: &[f64], rng: &mut StdRng| -> usize {
            choose_weighted_index(&weights.iter().map(|&w| w.powf(1.0 / TeamBuild::num_vars() as f64)).collect::<Vec<f64>>(), rng)
        };

        let mut random_pokemon_build = |poke_var_data: &PokeVarData, rng: &mut StdRng| -> PokemonBuild {
            let mut species = cwip(&poke_var_data.species.weights, rng) as SpeciesID;
            while used_species.contains(&species) {
                species = cwip(&poke_var_data.species.weights, rng) as SpeciesID;
            }
            if !Species::allow_duplicates(species) { used_species.insert(species); }

            let mut gender = Gender::by_id(cwip(&poke_var_data.gender.weights, rng) as u8);
            while !Species::can_be_gender(species, gender) {
                gender = Gender::by_id(cwip(&poke_var_data.gender.weights, rng) as u8);
            }

            let abilities = Species::abilities(species);
            let ability_weights: Vec<f64> = abilities.iter().map(|&a| poke_var_data.ability.weights[a as usize]).collect();

            let mut evs = [
                cwip(&poke_var_data.evs[0].weights, rng) as u8,
                cwip(&poke_var_data.evs[1].weights, rng) as u8,
                cwip(&poke_var_data.evs[2].weights, rng) as u8,
                cwip(&poke_var_data.evs[3].weights, rng) as u8,
                cwip(&poke_var_data.evs[4].weights, rng) as u8,
                cwip(&poke_var_data.evs[5].weights, rng) as u8
            ];
            while evs.iter().map(|&u| u as u16).sum::<u16>() != 510 {
                evs = [
                    cwip(&poke_var_data.evs[0].weights, rng) as u8,
                    cwip(&poke_var_data.evs[1].weights, rng) as u8,
                    cwip(&poke_var_data.evs[2].weights, rng) as u8,
                    cwip(&poke_var_data.evs[3].weights, rng) as u8,
                    cwip(&poke_var_data.evs[4].weights, rng) as u8,
                    cwip(&poke_var_data.evs[5].weights, rng) as u8
                ];
            }

            let move_pool = Species::move_pool(species);
            let move_weights: Vec<f64> = move_pool.iter().map(|&m| poke_var_data.moves.weights[m as usize]).collect();
            let mut moves= BTreeSet::new();
            while moves.len() < min(4, move_pool.len()) {
                moves.insert(move_pool[cwip(&move_weights, rng)]);
            }

            PokemonBuild {
                species,
                gender,
                nature: Nature::by_id(cwip(&poke_var_data.nature.weights, rng) as u8),
                ability: abilities[cwip(&ability_weights, rng)],
                ivs: [
                    cwip(&poke_var_data.ivs[0].weights, rng) as u8,
                    cwip(&poke_var_data.ivs[1].weights, rng) as u8,
                    cwip(&poke_var_data.ivs[2].weights, rng) as u8,
                    cwip(&poke_var_data.ivs[3].weights, rng) as u8,
                    cwip(&poke_var_data.ivs[4].weights, rng) as u8,
                    cwip(&poke_var_data.ivs[5].weights, rng) as u8
                ],
                evs,
                moves
            }
        };

        TeamBuild {
            party_leader: random_pokemon_build(&self.party_lead_data, rng),
            remaining_team: [
                random_pokemon_build(&self.remaining_team_data, rng),
                random_pokemon_build(&self.remaining_team_data, rng),
                random_pokemon_build(&self.remaining_team_data, rng),
                random_pokemon_build(&self.remaining_team_data, rng),
                random_pokemon_build(&self.remaining_team_data, rng)
            ]
        }
    }
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}
