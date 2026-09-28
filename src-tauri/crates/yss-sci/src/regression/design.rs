//! Shared numerical design preparation and row-ordered covariance projection.
use crate::error::invalid_input;
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
use yss_sci_linalg::{Mat, matrix_rank};

pub(crate) fn design_matrix(
    predictors: &[Vec<f64>],
    observations: usize,
    constant: bool,
    operation: SciOperationCode,
) -> Result<Mat<f64>, SciError> {
    if predictors.is_empty() {
        return Err(invalid_input(operation, SciInputViolation::EmptyInput));
    }
    if predictors.iter().any(|values| values.len() != observations) {
        return Err(invalid_input(operation, SciInputViolation::ShapeMismatch));
    }
    let columns = predictors.len() + usize::from(constant);
    Ok(Mat::from_fn(observations, columns, |row, column| {
        if constant && column == 0 {
            1.0
        } else {
            predictors[column - usize::from(constant)][row]
        }
    }))
}

pub(crate) fn covariance_rows(covariance: &Mat<f64>) -> Vec<Vec<f64>> {
    covariance
        .row_iter()
        .map(|row| row.iter().copied().collect())
        .collect()
}

pub(crate) fn design_condition_number(design: &Mat<f64>) -> f64 {
    matrix_rank(design.as_ref()).map_or(f64::INFINITY, |(_, condition)| condition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_and_covariance_keep_axis_order_at_report_boundary() {
        let design = design_matrix(
            &[vec![2.0, 3.0], vec![5.0, 7.0]],
            2,
            true,
            SciOperationCode::Regression,
        )
        .unwrap();
        assert_eq!(
            design,
            yss_sci_linalg::mat![[1.0, 2.0, 5.0], [1.0, 3.0, 7.0]]
        );
        let values = Mat::from_fn(2, 3, |row, column| (row * 3 + column) as f64);
        assert_eq!(
            covariance_rows(&values),
            vec![vec![0.0, 1.0, 2.0], vec![3.0, 4.0, 5.0]],
        );
    }
}
