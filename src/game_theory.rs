use std::f64;
use std::fmt::{Debug, Error, Formatter};

#[derive(Debug)]
pub struct ZeroSumNashEq {
    /// The maximizing player's strategy at equilibrium.
    pub max_player_strategy: Vec<f64>,
    /// The minimizing player's strategy at equilibrium.
    pub min_player_strategy: Vec<f64>,
    /// The expected payoff for the maximizing player at equilibrium.
    pub expected_payoff: f64,
}

/// Calculates the Nash equilibrium of a zero-sum payoff matrix.
/// The algorithm follows [Game Theory](docs/Game_Theory.pdf), section 4.5.
/// It requires that all elements be positive, so supply `added_constant` to ensure this; it will
/// not affect the equilibrium.
pub fn calc_nash_eq(payoff_matrix: &Matrix, row_domination: &[bool], col_domination: &[bool], added_constant: f64) -> ZeroSumNashEq {
    debug_assert!(row_domination.len() == payoff_matrix.num_rows() as usize && col_domination.len() == payoff_matrix.num_cols() as usize, "Domination labels must match the payoff matrix shape.");

    let m = payoff_matrix.num_rows() - row_domination.iter().filter(|b| **b).count();
    let n = payoff_matrix.num_cols() - col_domination.iter().filter(|b| **b).count();
    let mut tableau = Matrix::of(0.0, m + 1, n + 1);

    let mut i_t = 0;
    for (i, row_dominated) in row_domination.iter().enumerate() {
        if !row_dominated {
            let mut j_t = 0;
            for (j, col_dominated) in col_domination.iter().enumerate() {
                if !col_dominated {
                    *tableau.get_mut(i_t, j_t) = payoff_matrix.get(i, j) + added_constant;
                    j_t += 1;
                }
            }
            i_t += 1;
        }
    }

    tableau.set_col(n, 1.0);
    tableau.set_row(m, -1.0);
    *tableau.get_mut(m, n) = 0.0;

    // Row player's labels are positive
    let mut left_labels = vec![0; m];
    for (i, label) in left_labels.iter_mut().enumerate() {
        *label = i as i64 + 1;
    }

    // Column player's labels are negative
    let mut top_labels = vec![0; n];
    for (j, label) in top_labels.iter_mut().enumerate() {
        *label = -(j as i64 + 1);
    }

    let mut negative_remaining = true;
    while negative_remaining {
        let mut q = 0; // Column to pivot on
        for j in 1..n {
            if tableau.get(m, j) < tableau.get(m, q) { q = j; }
        }
        let mut p = 0; // Row to pivot on
        for possible_p in 0..m {
            let tppq = tableau.get(possible_p, q);
            let tpq = tableau.get(p, q);
            if !almost::zero(tppq) && tppq > 0.0 && (tableau.get(possible_p, n) / tppq < tableau.get(p, n) / tpq || almost::zero(tpq) || tpq < 0.0) {
                p = possible_p;
            }
        }

        // Pivot
        let pivot = tableau.get(p, q);
        for j in 0..(n + 1) {
            for i in 0..(m + 1) {
                if i != p && j != q { *tableau.get_mut(i, j) -= tableau.get(p, j) * tableau.get(i, q) / pivot; }
            }
        }
        for j in 0..(n + 1) {
            if j != q { *tableau.get_mut(p, j) /= pivot; }
        }
        for i in 0..(m + 1) {
            if i != p { *tableau.get_mut(i, q) /= -pivot; }
        }
        *tableau.get_mut(p, q) = 1.0 / pivot;

        // Exchange labels appropriately
        let temp = left_labels[p];
        left_labels[p] = top_labels[q];
        top_labels[q] = temp;

        negative_remaining = (0..n).any(|j| tableau.get(m, j) < 0.0);
    }

    let mut max_player_strategy = vec![0.0; m];
    let mut min_player_strategy = vec![0.0; n];
    for (j, &top_label) in top_labels.iter().enumerate() {
        if top_label > 0 { // If it's one of row player's labels
            max_player_strategy[top_label as usize - 1] = tableau.get(m, j) / tableau.get(m, n);
        }
    }
    for (i, &left_label) in left_labels.iter().enumerate() {
        if left_label < 0 { // If it's one of column player's labels
            min_player_strategy[-left_label as usize - 1] = tableau.get(i, n) / tableau.get(m, n);
        }
    }

    for (i, &row_dominated) in row_domination.iter().enumerate() {
        if row_dominated {
            max_player_strategy.insert(i, 0.0);
        }
    }
    for (j, &col_dominated) in col_domination.iter().enumerate() {
        if col_dominated {
            min_player_strategy.insert(j, 0.0);
        }
    }

    ZeroSumNashEq {
        max_player_strategy,
        min_player_strategy,
        expected_payoff: 1.0 / tableau.get(m, n) - added_constant,
    }
}

#[derive(Clone)]
pub struct Matrix {
    entries: Vec<f64>,
    num_rows: usize,
    num_cols: usize,
}

impl Matrix {
    pub fn from(entries: Vec<f64>, num_rows: usize, num_cols: usize) -> Matrix {
        debug_assert_ne!(num_rows as f64 * num_cols as f64 >= 2.0_f64.powf(16.0), true);
        debug_assert!(entries.len() == num_rows * num_cols, "Number of matrix entries does not match the specified dimensions.");
        Matrix {
            entries,
            num_rows,
            num_cols,
        }
    }

    pub fn of(fill: f64, num_rows: usize, num_cols: usize) -> Self {
        debug_assert_ne!(num_rows as f64 * num_cols as f64 >= 2.0_f64.powf(16.0), true);
        Matrix {
            entries: vec![fill; num_rows * num_cols],
            num_rows,
            num_cols,
        }
    }

    #[inline(always)]
    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    #[inline(always)]
    pub fn num_cols(&self) -> usize {
        self.num_cols
    }

    #[inline(always)]
    fn flat_index(&self, i: usize, j: usize) -> usize {
        debug_assert!(i < self.num_rows() && j < self.num_cols(), "Matrix indices out of bounds.");
        i * self.num_cols() + j
    }

    #[inline(always)]
    fn get(&self, i: usize, j: usize) -> f64 {
        self.entries[self.flat_index(i, j)]
    }

    #[inline(always)]
    pub fn get_mut(&mut self, i: usize, j: usize) -> &mut f64 {
        let flat_index = self.flat_index(i, j);
        self.entries.get_mut(flat_index).unwrap()
    }

    fn set_row(&mut self, i: usize, value: f64) {
        let flat_indices = (0..self.num_cols()).map(|j| self.flat_index(i, j)).collect::<Vec<usize>>();
        for flat_index in flat_indices {
            self.entries[flat_index] = value;
        }
    }

    fn set_col(&mut self, j: usize, value: f64) {
        let flat_indices = (0..self.num_rows()).map(|i| self.flat_index(i, j)).collect::<Vec<usize>>();
        for flat_index in flat_indices {
            self.entries[flat_index] = value;
        }
    }
}

impl Debug for Matrix {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut formatted = String::from("");
        self.entries.chunks(self.num_cols()).for_each(|row| {
            formatted.push_str(&format!("{:?}\n", row));
        });
        write!(f, "{}", formatted)
    }
}
