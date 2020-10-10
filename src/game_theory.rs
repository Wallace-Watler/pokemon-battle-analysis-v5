#[derive(Debug)]
pub struct ZeroSumNashEq {
    /// The maximizing player's strategy at equilibrium.
    pub max_player_strategy: Vec<f64>,
    /// The minimizing player's strategy at equilibrium.
    pub min_player_strategy: Vec<f64>,
    /// The expected payoff for the maximizing player at equilibrium.
    pub expected_payoff: f64
}

// TODO: Generify
pub struct Matrix {
    entries: Vec<f64>,
    num_rows: u16,
    num_cols: u16
}

impl Matrix {
    pub fn from(entries: Vec<f64>, num_rows: u16, num_cols: u16) -> Matrix {
        if entries.len() != (num_rows * num_cols) as usize { panic!("Number of matrix entries does not match the specified dimensions."); }
        Matrix {
            entries,
            num_rows,
            num_cols
        }
    }

    fn of(fill: f64, num_rows: u16, num_cols: u16) -> Matrix {
        Matrix {
            entries: vec![fill; (num_rows * num_cols) as usize],
            num_rows,
            num_cols
        }
    }

    fn flat_index(&self, i: u16, j: u16) -> usize {
        if i >= self.num_rows || j >= self.num_cols { panic!("Matrix indices out of bounds."); }
        (i * self.num_cols + j) as usize
    }

    fn get(&self, i: u16, j: u16) -> &f64 {
        unsafe {
            self.entries.get_unchecked(self.flat_index(i, j))
        }
    }

    fn get_mut(&mut self, i: u16, j: u16) -> &mut f64 {
        let flat_index = self.flat_index(i, j);
        unsafe {
            self.entries.get_unchecked_mut(flat_index)
        }
    }

    fn set_row(&mut self, i: u16, value: f64) {
        let flat_indices: Vec<usize> = (0..self.num_cols).map(|j| self.flat_index(i, j)).collect();
        for flat_index in flat_indices {
            unsafe {
                *self.entries.get_unchecked_mut(flat_index) = value;
            }
        }
    }

    fn set_col(&mut self, j: u16, value: f64) {
        let flat_indices: Vec<usize> = (0..self.num_rows).map(|i| self.flat_index(i, j)).collect();
        for flat_index in flat_indices {
            unsafe {
                *self.entries.get_unchecked_mut(flat_index) = value;
            }
        }
    }
}

/// Calculates the Nash equilibrium of a zero-sum payoff matrix.
/// The algorithm follows https://www.math.ucla.edu/~tom/Game_Theory/mat.pdf, section 4.5.
/// It requires that all elements be positive, so supply `added_constant` to ensure this; it will
/// not affect the equilibrium.
pub fn calc_nash_eq(payoff_matrix: &Matrix, added_constant: f64) -> ZeroSumNashEq {
    let m = payoff_matrix.num_rows;
    let n = payoff_matrix.num_cols;

    let mut tableau = Matrix::of(0.0, m + 1, n + 1);
    for i in 0..m {
        for j in 0..n {
            *tableau.get_mut(i, j) = *payoff_matrix.get(i, j) + added_constant;
        }
    }

    tableau.set_col(n, 1.0);
    tableau.set_row(m, -1.0);
    *tableau.get_mut(m, n) = 0.0;

    // Row player's labels are positive
    let mut left_labels = vec![0; m as usize];
    for (i, label) in left_labels.iter_mut().enumerate() {
        *label = i as i64 + 1;
    }

    // Column player's labels are negative
    let mut top_labels = vec![0; n as usize];
    for (j, label) in top_labels.iter_mut().enumerate() {
        *label = -(j as i64 + 1);
    }

    let mut negative_remaining = true;
    while negative_remaining {
        let mut q = 0; // Column to pivot on
        for j in 1..n {
            if *tableau.get(m, j) < *tableau.get(m, q) { q = j; }
        }
        let mut p = 0; // Row to pivot on
        for possible_p in 0..m {
            if *tableau.get(possible_p, q) > 1e-12 && (*tableau.get(possible_p, n) / *tableau.get(possible_p, q) < *tableau.get(p, n) / *tableau.get(p, q) || *tableau.get(p, q) <= 1e-12) {
                p = possible_p;
            }
        }

        // Pivot
        let pivot = *tableau.get(p, q);
        for j in 0..(n + 1) {
            for i in 0..(m + 1) {
                if i != p && j != q { *tableau.get_mut(i, j) -= *tableau.get(p, j) * *tableau.get(i, q) / pivot; }
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
        let temp = *left_labels.get(p as usize).unwrap();
        *left_labels.get_mut(p as usize).unwrap() = *top_labels.get(q as usize).unwrap();
        *top_labels.get_mut(q as usize).unwrap() = temp;

        negative_remaining = (0..n).any(|j| *tableau.get(m, j) < 0.0);
    }

    let mut max_player_strategy = vec![0.0; m as usize];
    let mut min_player_strategy = vec![0.0; n as usize];
    for j in 0..n {
        let top_label = *top_labels.get(j as usize).unwrap();
        if top_label > 0 { // If it's one of row player's labels
            *max_player_strategy.get_mut((top_label - 1) as usize).unwrap() = *tableau.get(m, j) / *tableau.get(m, n);
        }
    }
    for i in 0..m {
        let left_label = *left_labels.get(i as usize).unwrap();
        if left_label < 0 { // If it's one of column player's labels
            *min_player_strategy.get_mut((-left_label) as usize - 1).unwrap() = *tableau.get(i, n) / *tableau.get(m, n);
        }
    }

    ZeroSumNashEq {
        max_player_strategy,
        min_player_strategy,
        expected_payoff: 1.0 / *tableau.get(m, n) - added_constant,
    }
}
