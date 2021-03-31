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
