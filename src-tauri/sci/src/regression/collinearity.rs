//! Strict multicollinearity detection and column removal.
//!
//! When X has rank deficiency (strict multicollinearity), iteratively drop columns
//! until full rank. Removal priority: continuous > dummy > intercept.

use crate::tools::{IntoFaer, matrix_rank};
use ndarray::Array2;
use std::collections::BTreeSet;

/// Column type for removal priority: continuous (remove first), dummy, intercept (remove last).
fn removal_priority(j: usize, col_is_dummy: &[bool], intercept_col: Option<usize>) -> u8 {
    if intercept_col == Some(j) {
        2 // intercept: remove last
    } else if col_is_dummy.get(j).copied().unwrap_or(false) {
        1 // dummy: remove second
    } else {
        0 // continuous: remove first
    }
}

/// Drop strictly collinear columns from exog until full rank.
///
/// * `exog`: design matrix (n × k)
/// * `col_is_dummy`: for each column, true if dummy variable (categorical).
/// * `intercept_col`: column index of intercept (constant). If Some, that column is removed last.
///
/// Removal priority (first to remove): continuous > dummy > intercept.
///
/// Returns `(reduced_exog, omitted_indices)` where omitted_indices are the original
/// column indices that were dropped. If no collinearity, omitted_indices is empty.
pub fn drop_collinear_columns(
    exog: &Array2<f64>,
    col_is_dummy: &[bool],
    intercept_col: Option<usize>,
) -> Result<(Array2<f64>, Vec<usize>), String> {
    let _n = exog.nrows();
    let k = exog.ncols();
    if k == 0 {
        return Ok((exog.clone(), Vec::new()));
    }
    if col_is_dummy.len() != k {
        return Err(format!(
            "drop_collinear_columns: col_is_dummy len {} != exog cols {}",
            col_is_dummy.len(),
            k
        ));
    }

    let mut keep: BTreeSet<usize> = (0..k).collect();
    let mut omitted: Vec<usize> = Vec::new();

    // Removal order: continuous first, then dummy, then intercept. Within each group, lower index first.
    let mut removal_order: Vec<usize> = (0..k).collect();
    removal_order.sort_by(|&a, &b| {
        let pa = removal_priority(a, col_is_dummy, intercept_col);
        let pb = removal_priority(b, col_is_dummy, intercept_col);
        pa.cmp(&pb).then_with(|| a.cmp(&b))
    });

    loop {
        let keep_vec: Vec<usize> = keep.iter().copied().collect();
        let x_sub = exog.select(ndarray::Axis(1), &keep_vec);
        let x_faer = x_sub.view().into_faer().to_owned();
        let (rank, _) = matrix_rank(x_faer);

        if rank == keep.len() {
            break;
        }
        if rank >= keep.len() {
            break;
        }

        // Rank deficient: drop one column. Prefer one that yields full rank (rank_new == keep_new.len()).
        // If none does (deficiency > 1), drop any column in the span (rank_new == rank) and iterate.
        let mut dropped = false;
        for &j in &removal_order {
            if !keep.contains(&j) {
                continue;
            }
            let keep_new: Vec<usize> = keep.iter().copied().filter(|&i| i != j).collect();
            if keep_new.is_empty() {
                return Err("drop_collinear_columns: cannot drop all columns".to_string());
            }
            let x_new = exog.select(ndarray::Axis(1), &keep_new);
            let x_new_faer = x_new.view().into_faer().to_owned();
            let (rank_new, _) = matrix_rank(x_new_faer);

            if rank_new == keep_new.len() {
                // Full rank achieved with this removal
                keep.remove(&j);
                omitted.push(j);
                dropped = true;
                break;
            }
            if rank_new == rank {
                // Column j is in the span of others; dropping it reduces deficiency
                keep.remove(&j);
                omitted.push(j);
                dropped = true;
                break;
            }
        }
        if !dropped {
            return Err(
                "drop_collinear_columns: rank deficient but no single column removal yields full rank".to_string(),
            );
        }
    }

    let keep_vec: Vec<usize> = keep.iter().copied().collect();
    let reduced = exog.select(ndarray::Axis(1), &keep_vec).to_owned();
    Ok((reduced, omitted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_full_rank_no_drop() {
        let x = array![
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0]
        ];
        let is_dummy = vec![false, true, true];
        let (reduced, omitted) = drop_collinear_columns(&x, &is_dummy, Some(0)).unwrap();
        assert!(omitted.is_empty());
        assert_eq!(reduced.shape(), x.shape());
    }

    #[test]
    fn test_collinear_drop_non_dummy_first() {
        // col2 = col1, so rank 2. Prefer to drop col1 (continuous) over col2 (dummy)
        let x = array![[1.0, 1.0, 1.0], [1.0, 2.0, 2.0], [1.0, 3.0, 3.0],];
        let is_dummy = vec![false, false, true]; // const, x1, x2; x2=x1
        let (reduced, omitted) = drop_collinear_columns(&x, &is_dummy, Some(0)).unwrap();
        assert_eq!(omitted.len(), 1);
        assert_eq!(omitted[0], 1); // drop col1 (non-dummy) not col2 (dummy)
        assert_eq!(reduced.ncols(), 2);
    }

    #[test]
    fn test_multi_deficiency_drop_iteratively() {
        // const + 3 cols where col1=col2=col3 (all same), rank 2. Deficiency = 2.
        let x = array![
            [1.0, 1.0, 1.0, 1.0],
            [1.0, 2.0, 2.0, 2.0],
            [1.0, 3.0, 3.0, 3.0],
        ];
        let is_dummy = vec![false, false, false, true];
        let (reduced, omitted) = drop_collinear_columns(&x, &is_dummy, Some(0)).unwrap();
        assert_eq!(reduced.ncols(), 2); // const + one of col1/col2/col3
        assert_eq!(omitted.len(), 2);
    }
}
