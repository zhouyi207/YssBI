//! Linear hypothesis-test application API and Rust backend orchestration.
use yss_sci_contract::hypothesis::{Alternative, TTestResult, WaldTestResult};

use faer::{Col, Mat};

pub struct LinearHypothesisTestInput<'a> {
    pub betas: &'a Col<f64>,
    pub cov_beta: &'a Mat<f64>,
    pub r: &'a Mat<f64>,
    pub r_vec: &'a Col<f64>,
    pub df_residual: usize,
    pub alternative: Alternative,
    pub constraint_desc: String,
}

use yss_sci::stats::{t_test as yss_t_test, wald_test as yss_wald_test};

use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};

pub fn t_test(input: LinearHypothesisTestInput<'_>) -> Result<TTestResult, SciError> {
    validate_input(&input, SciOperationCode::TTest, true)?;
    yss_t_test(
        input.betas,
        input.cov_beta,
        input.r,
        input.r_vec,
        input.df_residual,
        input.alternative,
        input.constraint_desc,
    )
    .map_err(|_| SciError::ComputationFailed {
        operation: SciOperationCode::TTest,
    })
}

pub fn wald_test(input: LinearHypothesisTestInput<'_>) -> Result<WaldTestResult, SciError> {
    validate_input(&input, SciOperationCode::WaldTest, false)?;
    yss_wald_test(
        input.betas,
        input.cov_beta,
        input.r,
        input.r_vec,
        input.df_residual,
        input.alternative,
        input.constraint_desc,
    )
    .map_err(|_| SciError::ComputationFailed {
        operation: SciOperationCode::WaldTest,
    })
}

fn validate_input(
    input: &LinearHypothesisTestInput<'_>,
    operation: SciOperationCode,
    requires_single_constraint: bool,
) -> Result<(), SciError> {
    let constraint_count = input.r.nrows();
    let coefficient_count = input.r.ncols();
    if input.df_residual == 0
        || constraint_count == 0
        || (requires_single_constraint && constraint_count != 1)
    {
        return Err(invalid_input(
            operation,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    if input.betas.nrows() != coefficient_count
        || input.cov_beta.nrows() != coefficient_count
        || input.cov_beta.ncols() != coefficient_count
        || input.r_vec.nrows() != constraint_count
    {
        return Err(invalid_input(operation, SciInputViolation::ShapeMismatch));
    }
    if input
        .betas
        .iter()
        .chain(input.cov_beta.col_iter().flat_map(|column| column.iter()))
        .chain(input.r.col_iter().flat_map(|column| column.iter()))
        .chain(input.r_vec.iter())
        .any(|value| !value.is_finite())
    {
        return Err(invalid_input(operation, SciInputViolation::NonFiniteInput));
    }
    Ok(())
}

fn invalid_input(operation: SciOperationCode, violation: SciInputViolation) -> SciError {
    SciError::InvalidInput {
        operation,
        violation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::{Col, Mat, col, mat};

    fn input<'a>(
        betas: &'a Col<f64>,
        cov_beta: &'a Mat<f64>,
        r: &'a Mat<f64>,
        r_vec: &'a Col<f64>,
        df_residual: usize,
    ) -> LinearHypothesisTestInput<'a> {
        LinearHypothesisTestInput {
            betas,
            cov_beta,
            r,
            r_vec,
            df_residual,
            alternative: Alternative::TwoSided,
            constraint_desc: "review validation".to_owned(),
        }
    }

    #[test]
    fn hypothesis_input_validation_returns_specific_typed_violations() {
        let betas = col![1.0, 2.0];
        let covariance = mat![[0.1, 0.0], [0.0, 0.1]];
        let constraint = mat![[0.0, 1.0]];
        let target = col![0.0];

        assert_eq!(
            t_test(input(&betas, &covariance, &constraint, &target, 0)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::ParameterOutOfRange,
            }
        );

        let empty_constraints = Mat::zeros(0, 2);
        let empty_target = Col::zeros(0);
        assert_eq!(
            wald_test(input(
                &betas,
                &covariance,
                &empty_constraints,
                &empty_target,
                10,
            ))
            .unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::WaldTest,
                violation: SciInputViolation::ParameterOutOfRange,
            }
        );

        let short_betas = col![1.0];
        assert_eq!(
            t_test(input(&short_betas, &covariance, &constraint, &target, 10,)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::ShapeMismatch,
            }
        );

        let short_covariance = mat![[0.1]];
        assert_eq!(
            t_test(input(&betas, &short_covariance, &constraint, &target, 10,)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::ShapeMismatch,
            }
        );

        let long_target = col![0.0, 0.0];
        assert_eq!(
            wald_test(input(&betas, &covariance, &constraint, &long_target, 10,)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::WaldTest,
                violation: SciInputViolation::ShapeMismatch,
            }
        );

        let non_finite_betas = col![f64::NAN, 2.0];
        assert_eq!(
            t_test(input(
                &non_finite_betas,
                &covariance,
                &constraint,
                &target,
                10,
            ))
            .unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::NonFiniteInput,
            }
        );
    }

    #[test]
    fn hypothesis_numerical_failures_map_to_computation_failed() {
        let betas = col![1.0, 2.0];
        let target = col![0.0];
        let constraint = mat![[0.0, 1.0]];
        let zero_covariance = Mat::zeros(2, 2);
        assert_eq!(
            t_test(input(&betas, &zero_covariance, &constraint, &target, 10,)).unwrap_err(),
            SciError::ComputationFailed {
                operation: SciOperationCode::TTest,
            }
        );

        let constraints = mat![[1.0, 0.0], [0.0, 1.0]];
        let targets = col![0.0, 0.0];
        let singular_covariance = mat![[1.0, 1.0], [1.0, 1.0]];
        assert_eq!(
            wald_test(input(
                &betas,
                &singular_covariance,
                &constraints,
                &targets,
                10,
            ))
            .unwrap_err(),
            SciError::ComputationFailed {
                operation: SciOperationCode::WaldTest,
            }
        );
    }
}
