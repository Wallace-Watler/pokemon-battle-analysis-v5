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

    let m = payoff_matrix.num_rows() - row_domination.iter().filter(|b| **b).count() as u16;
    let n = payoff_matrix.num_cols() - col_domination.iter().filter(|b| **b).count() as u16;
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

// TODO: Create Tableau struct that contains a Matrix, a is_col_basis: Vec<bool>, and a basis_col: Vec<Option<usize>> (Option because a row can be all zeros)
#[derive(Clone)]
pub struct Matrix {
    entries: Vec<f64>,
    num_rows: u16, // TODO: make usize
    num_cols: u16 // TODO: make usize
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

    fn transposed(&self) -> Matrix {
        let mut result = Matrix::of(0.0, self.num_cols, self.num_rows);
        for i in 0..self.num_rows {
            for j in 0..self.num_cols {
                *result.get_mut(j, i) = self.get(i, j);
            }
        }
        result
    }

    fn without_row(&self, i: u16) -> Matrix {
        let mut result = self.clone();
        result.del_row(i);
        result
    }

    fn without_col(&self, j: u16) -> Matrix {
        let mut result = self.clone();
        result.del_col(j);
        result
    }

    pub fn scale(&mut self, factor: f64) {
        for entry in &mut self.entries {
            *entry *= factor;
        }
    }

    pub fn row_col_restricted(&self, row_exclusion: &[bool], col_exclusion: &[bool]) -> Matrix {
        if row_exclusion.len() != self.num_rows() as usize || col_exclusion.len() != self.num_cols() as usize {
            panic!("Row and column exclusions must match matrix dimensions.");
        }

        let m = self.num_rows() - row_exclusion.iter().filter(|b| **b).count() as u16;
        let n = self.num_cols() - col_exclusion.iter().filter(|b| **b).count() as u16;
        let mut result = Matrix::of(0.0, m, n);

        let mut i_r = 0;
        for i in 0..self.num_rows() {
            if !row_exclusion.get(i as usize).unwrap() {
                let mut j_r = 0;
                for j in 0..self.num_cols() {
                    if !col_exclusion.get(j as usize).unwrap() {
                        *result.get_mut(i_r, j_r) = self.get(i, j);
                        j_r += 1;
                    }
                }
                i_r += 1;
            }
        }

        result
    }

    pub const fn is_empty(&self) -> bool {
        self.num_rows == 0 || self.num_cols == 0
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

    pub fn del_row(&mut self, i: u16) {
        let del_from = self.flat_index(i, 0);
        let del_to = self.flat_index(i, self.num_cols - 1);
        self.entries.drain(del_from..=del_to);
        self.num_rows -= 1;
    }

    pub fn del_col(&mut self, j: u16) {
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

impl Display for Matrix {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let mut formatted = String::from("");
        self.entries.chunks(self.num_cols as usize).for_each(|row| {
            formatted.push_str(&format!("{:?}", row));
            formatted.push('\n');
        });
        write!(f, "{}", formatted)
    }
}

pub fn pivot_with_basis(tableau: &mut Matrix, pivot_row: u16, pivot_col: u16, basis: &mut [bool]) {
    if *basis.get(pivot_col as usize).unwrap() { return; }

    let mut exiting_var = 0;
    for j in 0..(tableau.num_cols() - 1) {
        if *basis.get(j as usize).unwrap() && !almost::zero(tableau.get(pivot_row, j)) {
            exiting_var = j as usize;
            break;
        }
    }
    *basis.get_mut(exiting_var).unwrap() = false;
    *basis.get_mut(pivot_col as usize).unwrap() = true;

    pivot(tableau, pivot_row, pivot_col);
}

fn pivot(tableau: &mut Matrix, pivot_row: u16, pivot_col: u16) {
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

pub fn select_pivot_col(tableau: &Matrix, basis: &[bool]) -> Option<u16> {
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
    if almost::zero(min_obj_coeff) || min_obj_coeff > 0.0 { None } else { pivot_col }
}

pub fn select_pivot_row(tableau: &Matrix, pivot_col: u16, min_row: u16) -> u16 {
    let mut pivot_row = min_row;
    let mut min_ratio = f64::INFINITY;
    for i in min_row..tableau.num_rows {
        if tableau.get(i, pivot_col) > 0.0 && !almost::zero(tableau.get(i, pivot_col)) {
            let ratio = tableau.get(i, tableau.num_cols - 1) / tableau.get(i, pivot_col);
            if ratio < min_ratio {
                min_ratio = ratio;
                pivot_row = i;
            }
        }
    }
    pivot_row
}

pub fn simplex_phase2(tableau: &mut Matrix, basis: &mut [bool]) {
    if basis.len() != tableau.num_cols() as usize - 1 {
        panic!("Number of basis labels should equal number of tableau columns - 1");
    }

    while let Some(pivot_col) = select_pivot_col(tableau, basis) {
        let pivot_row = select_pivot_row(tableau, pivot_col, 1);
        println!("{}, {}", pivot_row, pivot_col);
        pivot_with_basis(tableau, pivot_row, pivot_col, basis);
    }
}

pub fn simplex_phase1(a: &Matrix, b: &[f64], c: &[f64]) -> Option<(Matrix, Vec<bool>)> {
    let m = a.num_rows();
    let n = a.num_cols();

    // Create a tableau representing the LP with slack variables and artificial variables.
    let mut tableau = Matrix::of(0.0, m + 3, n + 2 * m + 2);
    let mut basis = vec![false; tableau.num_cols() as usize - 1];

    for j in 0..n {
        *tableau.get_mut(1, j) = -*c.get(j as usize).unwrap();
        *tableau.get_mut(1, tableau.num_rows() - 1) = 1.0;
        for i in 0..m {
            *tableau.get_mut(i + 2, j) = a.get(i, j);
        }
    }
    for i in 0..m {
        *tableau.get_mut(i + 2, tableau.num_cols() - 1) = *b.get(i as usize).unwrap();
    }
    for j in 0..m {
        *tableau.get_mut(j + 2, j + n) = 1.0;
        *basis.get_mut((j + n) as usize).unwrap() = true;
        *tableau.get_mut(0, j + n + m) = 1.0;
        *tableau.get_mut(j + 2, j + n + m) = if tableau.get(j + 2, tableau.num_cols() - 1) < 0.0 { -1.0 } else { 1.0 };
    }
    *tableau.get_mut(0, tableau.num_cols() - 2) = 1.0;
    *tableau.get_mut(tableau.num_rows() - 1, tableau.num_cols() - 2) = 1.0;
    *tableau.get_mut(tableau.num_rows() - 1, tableau.num_cols() - 1) = 1.0;

    print!("tableau:\n{}", tableau);
    println!("basis: {:?}\n", basis);

    // Pivot once on each artificial variable column.
    for i in 0..(m + 1) {
        pivot_with_basis(&mut tableau, i + 2, i + n + m, &mut basis);
    }

    print!("arti:\n{}", tableau);
    println!("basis: {:?}\n", basis);

    // Pivot normally until an optimum is reached.
    while let Some(pivot_col) = select_pivot_col(&mut tableau, &mut basis) {
        let pivot_row = select_pivot_row(&mut tableau, pivot_col, 2);
        println!("{}, {}", pivot_row, pivot_col);
        pivot_with_basis(&mut tableau, pivot_row, pivot_col, &mut basis);
    }

    print!("optimum:\n{}", tableau);
    println!("basis: {:?}\n", basis);

    // If the artificial variables are not zero by now, the original LP is infeasible.
    if !almost::zero(tableau.get(0, tableau.num_cols() - 1)) {
        return None;
    }

    // Check that all the artificial variables are non-basic.
    for j in 0..(m + 1) {
        if *basis.get((j + n + m) as usize).unwrap() {
            let mut pivot_row = 1;
            for possible_i in 2..tableau.num_rows() {
                if !almost::zero(tableau.get(pivot_row, j + n + m)) {
                    pivot_row = possible_i;
                }
            }
            // Pivot on some other non-basic variable with a positive entry in the pivot row.
            let mut pivoted = false;
            for pivot_col in 0..(n + m) {
                if !*basis.get(pivot_col as usize).unwrap() {
                    let potential_pivot = tableau.get(pivot_row, pivot_col);
                    if !almost::zero(potential_pivot) && potential_pivot > 0.0 {
                        pivot_with_basis(&mut tableau, pivot_row, pivot_col, &mut basis);
                        pivoted = true;
                        break;
                    }
                }
            }
            // If no such entry exists, the row is a redundant equation, so just delete it
            // and the basic artificial variable.
            if !pivoted {
                tableau.del_row(pivot_row);
                *basis.get_mut(j as usize).unwrap() = false;
            }
        }
    }

    print!("dropping:\n{}", tableau);
    println!("basis: {:?}\n", basis);

    // Drop the artificial variables, creating a canonical tableau equivalent to the original LP.
    tableau.del_row(0);
    for j in (0..(m + 1)).rev() {
        tableau.del_col(j + n + m);
    }

    print!("tableau:\n{}", tableau);
    println!("basis: {:?}\n", basis);

    Some((tableau, basis))
}

pub fn alpha_child(a: u16, b: u16, pessimistic_bounds_wo_domination: &Matrix, optimistic_bounds_wo_domination: &Matrix, alpha: f64) -> f64 {
    let mut p_t = pessimistic_bounds_wo_domination.clone();
    p_t.set_row(a, alpha);
    let e: Vec<f64> = (0..p_t.num_rows()).map(|i| p_t.get(i, b)).collect();
    p_t.del_col(b);
    p_t = p_t.transposed();
    p_t.scale(-1.0);

    let f: Vec<f64> = (0..optimistic_bounds_wo_domination.num_cols()).filter(|j| *j != b).map(|j| -optimistic_bounds_wo_domination.get(a, j)).collect();

    if let Some((mut tableau, mut basis)) = simplex_phase1(&p_t, &f, &e) {
        simplex_phase2(&mut tableau, &mut basis);
        print!("tableau:\n{}", tableau);
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

pub fn beta_child(a: u16, b: u16, pessimistic_bounds_wo_domination: &Matrix, optimistic_bounds_wo_domination: &Matrix, beta: f64) -> f64 {
    let mut o = optimistic_bounds_wo_domination.clone();
    o.set_col(b, beta);
    let e: Vec<f64> = (0..o.num_cols()).map(|j| -o.get(a, j)).collect();
    o.del_row(a);

    let f: Vec<f64> = (0..pessimistic_bounds_wo_domination.num_rows()).filter(|i| *i != a).map(|i| pessimistic_bounds_wo_domination.get(i, b)).collect();

    if let Some((mut tableau, mut basis)) = simplex_phase1(&o, &f, &e) {
        simplex_phase2(&mut tableau, &mut basis);
        print!("tableau:\n{}", tableau);
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
