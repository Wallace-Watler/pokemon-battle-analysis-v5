#[derive(Debug)]
pub struct ZeroSumNashEq {
    /// The maximizing player's strategy at equilibrium.
    pub max_player_strategy: Vec<f64>,
    /// The minimizing player's strategy at equilibrium.
    pub min_player_strategy: Vec<f64>,
    /// The expected payoff for the maximizing player at equilibrium.
    pub expected_payoff: f64
}

/// Calculates the Nash equilibrium of a zero-sum payoff matrix.
/// The algorithm follows [Game Theory](docs/Game_Theory.pdf), section 4.5.
/// It requires that all elements be positive, so supply `added_constant` to ensure this; it will
/// not affect the equilibrium.
pub fn calc_nash_eq(payoff_matrix: &Matrix, added_constant: f64) -> ZeroSumNashEq {
    let m = payoff_matrix.num_rows();
    let n = payoff_matrix.num_cols();

    let mut tableau = Matrix::of(0.0, m + 1, n + 1);
    for i in 0..m {
        for j in 0..n {
            *tableau.get_mut(i, j) = payoff_matrix.get(i, j) + added_constant;
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
            if tableau.get(m, j) < tableau.get(m, q) { q = j; }
        }
        let mut p = 0; // Row to pivot on
        for possible_p in 0..m {
            if tableau.get(possible_p, q) > 1e-12 && (tableau.get(possible_p, n) / tableau.get(possible_p, q) < tableau.get(p, n) / tableau.get(p, q) || tableau.get(p, q) <= 1e-12) {
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
        let temp = *left_labels.get(p as usize).unwrap();
        *left_labels.get_mut(p as usize).unwrap() = *top_labels.get(q as usize).unwrap();
        *top_labels.get_mut(q as usize).unwrap() = temp;

        negative_remaining = (0..n).any(|j| tableau.get(m, j) < 0.0);
    }

    let mut max_player_strategy = vec![0.0; m as usize];
    let mut min_player_strategy = vec![0.0; n as usize];
    for j in 0..n {
        let top_label = *top_labels.get(j as usize).unwrap();
        if top_label > 0 { // If it's one of row player's labels
            *max_player_strategy.get_mut((top_label - 1) as usize).unwrap() = tableau.get(m, j) / tableau.get(m, n);
        }
    }
    for i in 0..m {
        let left_label = *left_labels.get(i as usize).unwrap();
        if left_label < 0 { // If it's one of column player's labels
            *min_player_strategy.get_mut((-left_label) as usize - 1).unwrap() = tableau.get(i, n) / tableau.get(m, n);
        }
    }

    ZeroSumNashEq {
        max_player_strategy,
        min_player_strategy,
        expected_payoff: 1.0 / tableau.get(m, n) - added_constant,
    }
}

// TODO: Generify with copiable T (so that basis labels can be Matrix<bool>)
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

    pub fn of(fill: f64, num_rows: u16, num_cols: u16) -> Matrix {
        Matrix {
            entries: vec![fill; (num_rows * num_cols) as usize],
            num_rows,
            num_cols
        }
    }

    pub fn from_restricted_matrix(matrix: &Matrix, row_exclusion: &[bool], col_exclusion: &[bool]) -> Matrix {
        if row_exclusion.len() != matrix.num_rows() as usize || col_exclusion.len() != matrix.num_cols() as usize {
            panic!("Row and column exclusions must match matrix dimensions.");
        }

        let m = matrix.num_rows() - row_exclusion.iter().filter(|b| **b).count() as u16;
        let n = matrix.num_cols() - col_exclusion.iter().filter(|b| **b).count() as u16;
        let mut result = Matrix::of(0.0, m, n);
        let mut i_r = 0;
        let mut j_r = 0;

        for i in 0..matrix.num_rows() {
            if !row_exclusion.get(i as usize).unwrap() {
                for j in 0..matrix.num_cols() {
                    if !col_exclusion.get(j as usize).unwrap() {
                        *result.get_mut(i_r, j_r) = matrix.get(i, j);
                        j_r += 1;
                    }
                }
                i_r += 1;
            }
        }

        result
    }

    pub const fn num_rows(&self) -> u16 {
        self.num_rows
    }

    pub const fn num_cols(&self) -> u16 {
        self.num_cols
    }

    fn flat_index(&self, i: u16, j: u16) -> usize {
        if i >= self.num_rows || j >= self.num_cols { panic!("Matrix indices out of bounds."); }
        (i * self.num_cols + j) as usize
    }

    pub fn get(&self, i: u16, j: u16) -> f64 {
        unsafe {
            *self.entries.get_unchecked(self.flat_index(i, j))
        }
    }

    pub fn get_mut(&mut self, i: u16, j: u16) -> &mut f64 {
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

fn pivot(tableau: &mut Matrix, pivot_row: u16, pivot_col: u16, basis: &mut [bool]) {
    *basis.get_mut(pivot_col as usize).unwrap() = true;
    let pivot = tableau.get(pivot_row, pivot_col);
    for j in 0..(tableau.num_cols - 1) {
        if !almost::zero(tableau.get(pivot_row, j)) {
            *basis.get_mut(j as usize).unwrap() = false;
            break;
        }
    }
    for i in 0..tableau.num_rows {
        if i == pivot_row {
            for j in 0..tableau.num_cols {
                *tableau.get_mut(i, j) /= pivot;
            }
        } else if !almost::zero(tableau.get(i, pivot_col)) {
            for j in 0..tableau.num_cols {
                *tableau.get_mut(i, j) = tableau.get(i, j) / tableau.get(i, pivot_col) - tableau.get(pivot_row, j) / pivot;
            }
        }
    }
}

fn select_pivot_col(tableau: &Matrix, basis: &[bool]) -> Option<u16> {
    if basis.len() != tableau.num_cols as usize - 1 {
        panic!("Number of basis labels should equal number of tableau columns - 1");
    }
    let mut pivot_col = Some(1);
    let mut min_obj_coeff = tableau.get(0, 1);
    for j in 2..(tableau.num_cols - 1) {
        let obj_coeff = tableau.get(0, j);
        if !basis[j as usize] && obj_coeff < min_obj_coeff {
            min_obj_coeff = obj_coeff;
            pivot_col = Some(j);
        }
    }
    if min_obj_coeff > 0.0 { None } else { pivot_col }
}

fn select_pivot_row(tableau: &Matrix, pivot_col: u16) -> u16 {
    let mut pivot_row = 1;
    let mut min_ratio = tableau.get(1, tableau.num_cols - 1) / tableau.get(1, pivot_col);
    for i in 2..tableau.num_rows {
        if tableau.get(i, pivot_col) > 1e-12 {
            let ratio = tableau.get(i, tableau.num_cols - 1) / tableau.get(i, pivot_col);
            if ratio < min_ratio {
                min_ratio = ratio;
                pivot_row = i;
            }
        }
    }
    pivot_row
}

fn simplex_phase1(tableau: &mut Matrix, num_unknowns: u16, basis: &mut [bool]) -> Option<(Matrix, Vec<bool>)> {
    if basis.len() != tableau.num_cols() as usize - 1 {
        panic!("Number of basis labels should equal number of tableau columns - 1");
    }

    // Also the number of artificial variables.
    let num_slack_vars = tableau.num_rows() - 3;

    // Pivot once on each artificial variable column.
    for i in 2..(2 + num_slack_vars) {
        pivot(tableau, i, i + num_unknowns + num_slack_vars, basis);
    }

    // Pivot normally until an optimum is reached.
    while let Some(pivot_col) = select_pivot_col(tableau, basis) {
        let pivot_row = select_pivot_row(tableau, pivot_col);
        pivot(tableau, pivot_row, pivot_col, basis);
    }

    // If the artificial variables are not zero by now, the original LP is infeasible.
    if !almost::zero(tableau.get(0, tableau.num_cols() - 1)) {
        return None;
    }

    // Drop the artificial variables, creating a new canonical tableau equivalent to the original LP.
    let mut canonical_tableau = Matrix::of(0.0, tableau.num_rows() - 1, tableau.num_cols() - 1 - num_slack_vars);
    let mut canonical_basis = vec![false; canonical_tableau.num_cols() as usize - 1];
    for i in 0..canonical_tableau.num_rows() {
        for j in 0..(1 + num_unknowns + num_slack_vars) {
            *canonical_tableau.get_mut(i, j) = tableau.get(i + 1, j + 1);
            *canonical_basis.get_mut(j as usize).unwrap() = *basis.get(j as usize + 1).unwrap()
        }
        *canonical_tableau.get_mut(i, canonical_tableau.num_cols() - 1) = tableau.get(i + 1, tableau.num_cols() - 1);
    }

    Some((canonical_tableau, canonical_basis))
}

fn simplex_phase2(tableau: &mut Matrix, basis: &mut [bool]) {
    if basis.len() != tableau.num_cols() as usize - 1 {
        panic!("Number of basis labels should equal number of tableau columns - 1");
    }

    while let Some(pivot_col) = select_pivot_col(tableau, basis) {
        let pivot_row = select_pivot_row(tableau, pivot_col);
        pivot(tableau, pivot_row, pivot_col, basis);
    }
}

pub fn alpha_child(a: u16, b: u16, pessimistic_bounds_wo_domination: &Matrix, optimistic_bounds_wo_domination: &Matrix, alpha: f64) -> f64 {
    let m = pessimistic_bounds_wo_domination.num_rows(); // Also the number of unknowns
    let n = pessimistic_bounds_wo_domination.num_cols();
    let num_slack_vars = n - 1; // Also the number of artificial variables

    let mut tableau = Matrix::of(0.0, n + 2, 2 * num_slack_vars + m + 3);
    *tableau.get_mut(0, 0) = 1.0;
    *tableau.get_mut(1, 1) = 1.0;
    *tableau.get_mut(1, m + 1) = -alpha;
    *tableau.get_mut(n + 1, m + 1) = 1.0;
    *tableau.get_mut(n + 1, 2 * num_slack_vars + m + 2) = 1.0;

    for i in 0..m {
        if i < a {
            *tableau.get_mut(1, i + 2) = -pessimistic_bounds_wo_domination.get(i, b);
            *tableau.get_mut(n + 1, i + 2) = 1.0;
        } else if i > a {
            *tableau.get_mut(1, i + 1) = -pessimistic_bounds_wo_domination.get(i, b);
            *tableau.get_mut(n + 1, i + 1) = 1.0;
        }
    }

    for j in 0..num_slack_vars {
        *tableau.get_mut(0, 2 + m + num_slack_vars + j) = 1.0;
        *tableau.get_mut(j + 2, 2 + m + num_slack_vars + j) = 1.0;
    }

    for j in 0..n {
        if j < b {
            for i in 0..m {
                if i < a {
                    *tableau.get_mut(j + 2, i + 2) = pessimistic_bounds_wo_domination.get(i, j);
                } else if i > a {
                    *tableau.get_mut(j + 2, i + 1) = pessimistic_bounds_wo_domination.get(i, j);
                }
            }
            *tableau.get_mut(j + 2, m + 1) = alpha;
            *tableau.get_mut(j + 2, m + 2 + j) = -1.0;
            *tableau.get_mut(j + 2, 2 * num_slack_vars + m + 2) = optimistic_bounds_wo_domination.get(a, j);
        } else if j > b {
            for i in 0..m {
                if i < a {
                    *tableau.get_mut(j + 1, i + 2) = pessimistic_bounds_wo_domination.get(i, j);
                } else if i > a {
                    *tableau.get_mut(j + 1, i + 1) = pessimistic_bounds_wo_domination.get(i, j);
                }
            }
            *tableau.get_mut(j + 1, m + 1) = alpha;
            *tableau.get_mut(j + 1, m + 1 + j) = -1.0;
            *tableau.get_mut(j + 1, 2 * num_slack_vars + m + 2) = optimistic_bounds_wo_domination.get(a, j);
        }
    }

    let mut basis = vec![false; tableau.num_cols() as usize - 1];
    *basis.get_mut(0).unwrap() = true;
    *basis.get_mut(1).unwrap() = true;
    for j in (m as usize + 2)..(m + 2 + num_slack_vars) as usize {
        *basis.get_mut(j).unwrap() = true;
    }

    if let Some((mut canonical_tableau, mut canonical_basis)) = simplex_phase1(&mut tableau, m, &mut basis) {
        simplex_phase2(&mut canonical_tableau, &mut canonical_basis);
        canonical_tableau.get(0, canonical_tableau.num_cols() - 1)
    } else {
        -1.0
    }
}

pub fn beta_child(a: u16, b: u16, pessimistic_bounds_wo_domination: &Matrix, optimistic_bounds_wo_domination: &Matrix, beta: f64) -> f64 {
    let m = optimistic_bounds_wo_domination.num_rows();
    let n = optimistic_bounds_wo_domination.num_cols(); // Also the number of unknowns
    let num_slack_vars = m - 1; // Also the number of artificial variables

    let mut tableau = Matrix::of(0.0, m + 2, 2 * num_slack_vars + n + 3);
    *tableau.get_mut(0, 0) = 1.0;
    *tableau.get_mut(1, 1) = 1.0;
    *tableau.get_mut(1, n + 1) = -beta;
    *tableau.get_mut(m + 1, n + 1) = 1.0;
    *tableau.get_mut(m + 1, 2 * num_slack_vars + n + 2) = 1.0;

    for j in 0..n {
        if j < b {
            *tableau.get_mut(1, j + 2) = -optimistic_bounds_wo_domination.get(a, j);
            *tableau.get_mut(m + 1, j + 2) = 1.0;
        } else if j > b {
            *tableau.get_mut(1, j + 1) = -optimistic_bounds_wo_domination.get(a, j);
            *tableau.get_mut(m + 1, j + 1) = 1.0;
        }
    }

    for j in 0..num_slack_vars {
        *tableau.get_mut(0, 2 + n + num_slack_vars + j) = 1.0;
        *tableau.get_mut(j + 2, 2 + n + num_slack_vars + j) = -1.0;
    }

    for i in 0..m {
        if i < a {
            for j in 0..n {
                if j < b {
                    *tableau.get_mut(i + 2, j + 2) = optimistic_bounds_wo_domination.get(i, j);
                } else if j > b {
                    *tableau.get_mut(i + 2, j + 1) = optimistic_bounds_wo_domination.get(i, j);
                }
            }
            *tableau.get_mut(i + 2, n + 1) = beta;
            *tableau.get_mut(i + 2, n + 2 + i) = 1.0;
            *tableau.get_mut(i + 2, 2 * num_slack_vars + n + 2) = pessimistic_bounds_wo_domination.get(i, b);
        } else if i > a {
            for j in 0..n {
                if j < b {
                    *tableau.get_mut(i + 1, j + 2) = optimistic_bounds_wo_domination.get(i, j);
                } else if j > b {
                    *tableau.get_mut(i + 1, j + 1) = optimistic_bounds_wo_domination.get(i, j);
                }
            }
            *tableau.get_mut(i + 1, n + 1) = beta;
            *tableau.get_mut(i + 1, n + 1 + i) = 1.0;
            *tableau.get_mut(i + 1, 2 * num_slack_vars + n + 2) = pessimistic_bounds_wo_domination.get(i, b);
        }
    }

    let mut basis = vec![false; tableau.num_cols() as usize - 1];
    *basis.get_mut(0).unwrap() = true;
    *basis.get_mut(1).unwrap() = true;
    for j in (n as usize + 2)..(n + 2 + num_slack_vars) as usize {
        *basis.get_mut(j).unwrap() = true;
    }

    if let Some((mut canonical_tableau, mut canonical_basis)) = simplex_phase1(&mut tableau, n, &mut basis) {
        simplex_phase2(&mut canonical_tableau, &mut canonical_basis);
        -canonical_tableau.get(0, canonical_tableau.num_cols() - 1) // TODO: Should this be negative?
    } else {
        1.0
    }
}
