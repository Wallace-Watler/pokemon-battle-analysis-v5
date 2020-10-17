use std::fmt::{Display, Formatter, Error};
use std::f64;

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
pub fn calc_nash_eq(payoff_matrix: &Matrix, row_domination: &[bool], col_domination: &[bool], added_constant: f64) -> ZeroSumNashEq {
    if row_domination.len() != payoff_matrix.num_rows() as usize || col_domination.len() != payoff_matrix.num_cols() as usize {
        panic!("Domination labels must match the payoff matrix shape.");
    }

    let m = payoff_matrix.num_rows() - row_domination.iter().filter(|b| **b).count();
    let n = payoff_matrix.num_cols() - col_domination.iter().filter(|b| **b).count();
    let mut tableau = Matrix::of(0.0, m + 1, n + 1);

    let mut i_t = 0;
    for i in 0..payoff_matrix.num_rows() {
        if !row_domination.get(i as usize).unwrap() {
            let mut j_t = 0;
            for j in 0..payoff_matrix.num_cols() {
                if !col_domination.get(j as usize).unwrap() {
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

    for i in 0..row_domination.len() {
        if *row_domination.get(i).unwrap() {
            max_player_strategy.insert(i, 0.0);
        }
    }
    for j in 0..col_domination.len() {
        if *col_domination.get(j).unwrap() {
            min_player_strategy.insert(j, 0.0);
        }
    }

    ZeroSumNashEq {
        max_player_strategy,
        min_player_strategy,
        expected_payoff: 1.0 / tableau.get(m, n) - added_constant,
    }
}

pub trait IsMatrix: Display {
    fn entries(&self) -> &[f64];
    fn entries_mut(&mut self) -> &mut [f64];
    fn num_rows(&self) -> usize;
    fn num_cols(&self) -> usize;

    fn flat_index(&self, i: usize, j: usize) -> usize {
        if i >= self.num_rows() || j >= self.num_cols() { panic!("Matrix indices out of bounds."); }
        i * self.num_cols() + j
    }

    fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    fn get(&self, i: usize, j: usize) -> f64 {
        self.entries()[self.flat_index(i, j)]
    }

    fn get_mut(&mut self, i: usize, j: usize) -> &mut f64 {
        let flat_index = self.flat_index(i, j);
        self.entries_mut().get_mut(flat_index).unwrap()
    }

    fn set_row(&mut self, i: usize, value: f64) {
        let flat_indices = (0..self.num_cols()).map(|j| self.flat_index(i, j)).collect::<Vec<usize>>();
        for flat_index in flat_indices {
            *self.entries_mut().get_mut(flat_index).unwrap() = value;
        }
    }

    fn set_col(&mut self, j: usize, value: f64) {
        let flat_indices = (0..self.num_rows()).map(|i| self.flat_index(i, j)).collect::<Vec<usize>>();
        for flat_index in flat_indices {
            *self.entries_mut().get_mut(flat_index).unwrap() = value;
        }
    }

    fn scale(&mut self, factor: f64) {
        for entry in self.entries_mut() {
            *entry *= factor;
        }
    }

    fn pivot(&mut self, pivot_row: usize, pivot_col: usize) where Self: Sized {
        regular_pivot(self, pivot_row, pivot_col);
    }
}

fn regular_pivot(matrix: &mut dyn IsMatrix, pivot_row: usize, pivot_col: usize) {
    let pivot = matrix.get(pivot_row, pivot_col);
    if almost::zero(pivot) {
        println!("{}", matrix);
        panic!(format!("Pivot element ({}, {}) is zero.", pivot_row, pivot_col));
    }

    for j in 0..matrix.num_cols() {
        *matrix.get_mut(pivot_row, j) /= pivot;
    }
    for i in 0..matrix.num_rows() {
        if i != pivot_row {
            let tableau_i_pivot_col = matrix.get(i, pivot_col);
            if !almost::zero(tableau_i_pivot_col) {
                for j in 0..matrix.num_cols() {
                    *matrix.get_mut(i, j) = matrix.get(i, j) - matrix.get(pivot_row, j) * tableau_i_pivot_col
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Tableau {
    matrix: Matrix,
    /// Whether each column of the matrix is a basis column; length is always `matrix.num_cols() - 1`.
    col_is_basis: Vec<bool>,
    /// The basis column for each row; length is always `num_rows`.
    basis_col: Vec<Option<usize>>
}

impl Tableau {
    pub fn from(matrix: Matrix, col_is_basis: Vec<bool>, basis_col: Vec<Option<usize>>) -> Tableau {
        if col_is_basis.len() != matrix.num_cols() - 1 { panic!("Number of column basis flags does not equal the matrix's number of columns - 1."); }
        if basis_col.len() != matrix.num_rows() { panic!("Number of basis labels does not match the matrix's number of rows."); }
        Tableau {
            matrix,
            col_is_basis,
            basis_col
        }
    }

    pub fn col_is_basis(&self) -> &[bool] {
        &self.col_is_basis
    }

    pub fn col_is_basis_mut(&mut self) -> &mut [bool] {
        &mut self.col_is_basis
    }

    pub fn basis_col(&self) -> &[Option<usize>] {
        &self.basis_col
    }

    pub fn del_row(&mut self, i: usize) {
        self.matrix.del_row(i);
        if let Some(basis_col) = *self.basis_col().get(i).unwrap() {
            *self.col_is_basis_mut().get_mut(basis_col).unwrap() = false;
        }
        self.basis_col.remove(i);
    }

    pub fn del_col(&mut self, j: usize) {
        self.matrix.del_col(j);
        self.col_is_basis.remove(j);
        for i in 0..self.basis_col.len() {
            if self.basis_col[i] == Some(j) {
                self.basis_col[i] = None;
            }
        }
    }
}

impl IsMatrix for Tableau {
    fn entries(&self) -> &[f64] {
        self.matrix.entries()
    }

    fn entries_mut(&mut self) -> &mut [f64] {
        self.matrix.entries_mut()
    }

    fn num_rows(&self) -> usize {
        self.matrix.num_rows()
    }

    fn num_cols(&self) -> usize {
        self.matrix.num_cols()
    }

    fn pivot(&mut self, pivot_row: usize, pivot_col: usize) {
        if self.col_is_basis[pivot_col] { return; }

        let mut exiting_var = 0;
        for j in 0..(self.num_cols() - 1) {
            if self.col_is_basis[j] && !almost::zero(self.get(pivot_row, j)) {
                exiting_var = j;
                break;
            }
        }
        self.col_is_basis[exiting_var] = false;
        self.col_is_basis[pivot_col] = true;
        self.basis_col[pivot_row] = Some(pivot_col);

        regular_pivot(self, pivot_row, pivot_col);
    }
}

impl Display for Tableau {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut formatted = format!("{}", self.matrix);
        formatted.push_str(&format!("col_is_basis: {:?}\n", self.col_is_basis()));
        formatted.push_str(&format!("basis_col: {:?}\n", self.basis_col()));
        write!(f, "{}", formatted)
    }
}

#[derive(Clone)]
pub struct Matrix {
    entries: Vec<f64>,
    num_rows: usize,
    num_cols: usize
}

impl Matrix {
    pub fn from(entries: Vec<f64>, num_rows: usize, num_cols: usize) -> Matrix {
        if num_rows as f64 * num_cols as f64 >= 2.0_f64.powf(16.0) { panic!("Requested matrix size is too large."); }
        if entries.len() != num_rows * num_cols { panic!("Number of matrix entries does not match the specified dimensions."); }
        Matrix {
            entries,
            num_rows,
            num_cols
        }
    }

    pub fn of(fill: f64, num_rows: usize, num_cols: usize) -> Matrix {
        if num_rows as f64 * num_cols as f64 >= 2.0_f64.powf(16.0) { panic!("Requested matrix size is too large."); }
        Matrix {
            entries: vec![fill; num_rows * num_cols],
            num_rows,
            num_cols
        }
    }

    fn transposed(&self) -> Matrix {
        let mut result = Matrix::of(0.0, self.num_cols, self.num_rows);
        for i in 0..self.num_rows {
            for j in 0..self.num_cols {
                *result.get_mut(j, i) = self.get(i, j);
            }
        }
        result
    }

    fn without_row(&self, i: usize) -> Matrix {
        let mut result = self.clone();
        result.del_row(i);
        result
    }

    fn without_col(&self, j: usize) -> Matrix {
        let mut result = self.clone();
        result.del_col(j);
        result
    }

    pub fn row_col_restricted(&self, row_exclusion: &[bool], col_exclusion: &[bool]) -> Matrix {
        if row_exclusion.len() != self.num_rows() || col_exclusion.len() != self.num_cols() {
            panic!("Row and column exclusions must match matrix dimensions.");
        }

        let m = self.num_rows() - row_exclusion.iter().filter(|b| **b).count();
        let n = self.num_cols() - col_exclusion.iter().filter(|b| **b).count();
        let mut result = Matrix::of(0.0, m, n);

        let mut i_r = 0;
        for i in 0..self.num_rows() {
            if !row_exclusion.get(i).unwrap() {
                let mut j_r = 0;
                for j in 0..self.num_cols() {
                    if !col_exclusion.get(j).unwrap() {
                        *result.get_mut(i_r, j_r) = self.get(i, j);
                        j_r += 1;
                    }
                }
                i_r += 1;
            }
        }

        result
    }

    pub fn del_row(&mut self, i: usize) {
        let del_from = self.flat_index(i, 0);
        let del_to = self.flat_index(i, self.num_cols - 1);
        self.entries.drain(del_from..=del_to);
        self.num_rows -= 1;
    }

    pub fn del_col(&mut self, j: usize) {
        let del_from = self.flat_index(0, j) as isize;
        let del_to = self.flat_index(self.num_rows - 1, j) as isize;
        let mut del_index = del_to;
        while del_index >= del_from {
            self.entries.remove(del_index as usize);
            del_index -= self.num_cols as isize;
        }
        self.num_cols -= 1;
    }
}

impl IsMatrix for Matrix {
    fn entries(&self) -> &[f64] {
        &self.entries
    }

    fn entries_mut(&mut self) -> &mut [f64] {
        &mut self.entries
    }

    fn num_rows(&self) -> usize {
        self.num_rows
    }

    fn num_cols(&self) -> usize {
        self.num_cols
    }
}

impl Display for Matrix {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut formatted = String::from("");
        self.entries().chunks(self.num_cols()).for_each(|row| {
            formatted.push_str(&format!("{:?}\n", row));
        });
        write!(f, "{}", formatted)
    }
}

pub fn pivot_with_basis(tableau: &mut Matrix, pivot_row: usize, pivot_col: usize, basis: &mut [bool]) {
    if *basis.get(pivot_col).unwrap() { return; }

    let mut exiting_var = 0;
    for j in 0..(tableau.num_cols() - 1) {
        if *basis.get(j).unwrap() && !almost::zero(tableau.get(pivot_row, j)) {
            exiting_var = j;
            break;
        }
    }
    *basis.get_mut(exiting_var).unwrap() = false;
    *basis.get_mut(pivot_col).unwrap() = true;

    pivot(tableau, pivot_row, pivot_col);
}

fn pivot(tableau: &mut Matrix, pivot_row: usize, pivot_col: usize) {
    let pivot = tableau.get(pivot_row, pivot_col);
    for j in 0..tableau.num_cols {
        *tableau.get_mut(pivot_row, j) /= pivot;
    }
    for i in 0..tableau.num_rows {
        if i != pivot_row {
            let tableau_i_pivot_col = tableau.get(i, pivot_col);
            if !almost::zero(tableau_i_pivot_col) {
                for j in 0..tableau.num_cols {
                    *tableau.get_mut(i, j) = tableau.get(i, j) - tableau.get(pivot_row, j) * tableau_i_pivot_col
                }
            }
        }
    }
}

pub fn select_pivot_col(tableau: &Tableau) -> Option<usize> {
    let mut pivot_col = Some(0);
    let mut min_obj_coeff = tableau.get(0, 0);
    for j in 1..tableau.col_is_basis().len() {
        let obj_coeff = tableau.get(0, j);
        if !tableau.col_is_basis()[j] && obj_coeff < min_obj_coeff {
            min_obj_coeff = obj_coeff;
            pivot_col = Some(j);
        }
    }
    if almost::zero(min_obj_coeff) || min_obj_coeff > 0.0 { None } else { pivot_col }
}

pub fn select_pivot_row(tableau: &Tableau, pivot_col: usize, min_row: usize) -> usize {
    let mut pivot_row = min_row;
    let mut min_ratio = f64::INFINITY;
    for i in min_row..tableau.num_rows() {
        if tableau.get(i, pivot_col) > 0.0 && !almost::zero(tableau.get(i, pivot_col)) {
            let ratio = tableau.get(i, tableau.num_cols() - 1) / tableau.get(i, pivot_col);
            if ratio < min_ratio {
                min_ratio = ratio;
                pivot_row = i;
            }
        }
    }
    pivot_row
}

pub fn simplex_phase2(tableau: &mut Tableau) {
    while let Some(pivot_col) = select_pivot_col(tableau) {
        let pivot_row = select_pivot_row(tableau, pivot_col, 1);
        tableau.pivot(pivot_row, pivot_col);
    }
}

pub fn simplex_phase1(a: &Matrix, b: &[f64], c: &[f64]) -> Option<Tableau> {
    let m = a.num_rows();
    let n = a.num_cols();

    // Create a tableau representing the LP with slack variables and artificial variables.
    let mut matrix = Matrix::of(0.0, m + 3, n + 2 * m + 2);
    let mut is_col_basis = vec![false; matrix.num_cols() - 1];
    let mut basis_col = vec![None; matrix.num_rows()];

    for j in 0..n {
        *matrix.get_mut(1, j) = -*c.get(j).unwrap();
        *matrix.get_mut(matrix.num_rows() - 1, j) = 1.0;
        for i in 0..m {
            *matrix.get_mut(i + 2, j) = a.get(i, j);
        }
    }
    for i in 0..m {
        *matrix.get_mut(i + 2, matrix.num_cols() - 1) = *b.get(i).unwrap();
    }
    for j in 0..m {
        *matrix.get_mut(j + 2, j + n) = 1.0;
        *is_col_basis.get_mut(j + n).unwrap() = true;
        *basis_col.get_mut(j + 2).unwrap() = Some(j + n);
        *matrix.get_mut(0, j + n + m) = 1.0;
        *matrix.get_mut(j + 2, j + n + m) = if matrix.get(j + 2, matrix.num_cols() - 1) < 0.0 { -1.0 } else { 1.0 };
    }
    *matrix.get_mut(0, matrix.num_cols() - 2) = 1.0;
    *matrix.get_mut(matrix.num_rows() - 1, matrix.num_cols() - 2) = 1.0;
    *matrix.get_mut(matrix.num_rows() - 1, matrix.num_cols() - 1) = 1.0;

    let mut tableau = Tableau::from(matrix, is_col_basis, basis_col);

    // Pivot once on each artificial variable column.
    for i in 0..(m + 1) {
        tableau.pivot(i + 2, i + n + m);
    }

    // Pivot normally until an optimum is reached.
    while let Some(pivot_col) = select_pivot_col(&mut tableau) {
        let pivot_row = select_pivot_row(&mut tableau, pivot_col, 2);
        tableau.pivot(pivot_row, pivot_col);
    }

    // If the artificial variables are not zero by now, the original LP is infeasible.
    if !almost::zero(tableau.get(0, tableau.num_cols() - 1)) {
        return None;
    }

    // Check that all the artificial variables are non-basic.
    // TODO: Just check tableau.basis_col() for indices in the range (n + m)..(2 * m + 1 + n)
    for j in 0..(m + 1) {
        if *tableau.col_is_basis().get(j + n + m).unwrap() {
            let mut pivot_row = 1;
            for possible_i in 2..tableau.num_rows() {
                if !almost::zero(tableau.get(pivot_row, j + n + m)) {
                    pivot_row = possible_i;
                }
            }
            // Pivot on some other non-basic variable with a positive entry in the pivot row.
            let mut pivoted = false;
            for pivot_col in 0..(n + m) {
                if !*tableau.col_is_basis().get(pivot_col).unwrap() {
                    let potential_pivot = tableau.get(pivot_row, pivot_col);
                    if !almost::zero(potential_pivot) && potential_pivot > 0.0 {
                        tableau.pivot(pivot_row, pivot_col);
                        pivoted = true;
                        break;
                    }
                }
            }
            // If no such entry exists, the row is a redundant equation, so just delete it
            // and the basic artificial variable.
            if !pivoted {
                tableau.del_row(pivot_row);
                *tableau.col_is_basis_mut().get_mut(j).unwrap() = false;
            }
        }
    }

    // Drop the artificial variables, creating a canonical tableau equivalent to the original LP.
    tableau.del_row(0);
    for j in (0..(m + 1)).rev() {
        tableau.del_col(j + n + m);
    }

    Some(tableau)
}

pub fn alpha_child(a: usize, b: usize, pessimistic_bounds_wo_domination: &Matrix, optimistic_bounds_wo_domination: &Matrix, alpha: f64) -> f64 {
    let mut p_t = pessimistic_bounds_wo_domination.clone();
    p_t.set_row(a, alpha);
    let e: Vec<f64> = (0..p_t.num_rows()).map(|i| p_t.get(i, b)).collect();
    p_t.del_col(b);
    p_t = p_t.transposed();
    p_t.scale(-1.0);

    let f: Vec<f64> = (0..optimistic_bounds_wo_domination.num_cols()).filter(|j| *j != b).map(|j| -optimistic_bounds_wo_domination.get(a, j)).collect();

    if let Some(mut tableau) = simplex_phase1(&p_t, &f, &e) {
        simplex_phase2(&mut tableau);
        let mut alpha_child = tableau.get(0, tableau.num_cols() - 1);
        if almost::equal(alpha_child, -1.0) {
            alpha_child = -1.0;
        } else if almost::equal(alpha_child, 1.0) {
            alpha_child = 1.0;
        }
        if alpha_child < -1.0 || alpha_child > 1.0 {
            panic!(format!("Alpha outside of bounds: {}", alpha_child));
        }
        alpha_child
    } else {
        -1.0
    }
}

pub fn beta_child(a: usize, b: usize, pessimistic_bounds_wo_domination: &Matrix, optimistic_bounds_wo_domination: &Matrix, beta: f64) -> f64 {
    let mut o = optimistic_bounds_wo_domination.clone();
    o.set_col(b, beta);
    let e: Vec<f64> = (0..o.num_cols()).map(|j| -o.get(a, j)).collect();
    o.del_row(a);

    let f: Vec<f64> = (0..pessimistic_bounds_wo_domination.num_rows()).filter(|i| *i != a).map(|i| pessimistic_bounds_wo_domination.get(i, b)).collect();

    if let Some(mut tableau) = simplex_phase1(&o, &f, &e) {
        simplex_phase2(&mut tableau);
        let mut beta_child = -tableau.get(0, tableau.num_cols() - 1);
        if almost::equal(beta_child, -1.0) {
            beta_child = -1.0;
        } else if almost::equal(beta_child, 1.0) {
            beta_child = 1.0;
        }
        if beta_child < -1.0 || beta_child > 1.0 {
            panic!(format!("Beta outside of bounds: {}", beta_child));
        }
        beta_child
    } else {
        1.0
    }
}
