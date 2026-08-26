use yss_sci::stats::{
    Alternative as YssSciAlternative, t_test as yss_t_test, wald_test as yss_wald_test,
};

use crate::sci::api::stats::hypothesis::{
    Alternative, LinearHypothesisTestInput, TTestOutput, WaldTestOutput,
};
use crate::sci::error::{SciError, SciInputViolation, SciOperationCode};

pub fn t_test(input: LinearHypothesisTestInput<'_>) -> Result<TTestOutput, SciError> {
    validate_input(&input, SciOperationCode::TTest, true)?;
    let result = yss_t_test(
        input.betas,
        input.cov_beta,
        input.r,
        input.r_vec,
        input.df_residual,
        convert_alternative(input.alternative),
        input.constraint_desc,
    )
    .map_err(|_| SciError::ComputationFailed {
        operation: SciOperationCode::TTest,
    })?;

    Ok(TTestOutput {
        alternative: result.alternative,
        r_beta_minus_r: result.r_beta_minus_r,
        stat: result.stat,
        df: result.df,
        p_value: result.p_value,
    })
}

pub fn wald_test(input: LinearHypothesisTestInput<'_>) -> Result<WaldTestOutput, SciError> {
    validate_input(&input, SciOperationCode::WaldTest, false)?;
    let result = yss_wald_test(
        input.betas,
        input.cov_beta,
        input.r,
        input.r_vec,
        input.df_residual,
        convert_alternative(input.alternative),
        input.constraint_desc,
    )
    .map_err(|_| SciError::ComputationFailed {
        operation: SciOperationCode::WaldTest,
    })?;

    Ok(WaldTestOutput {
        alternative: result.alternative,
        r_beta_minus_r: result.r_beta_minus_r,
        stat: result.stat,
        df1: result.df1,
        df2: result.df2,
        p_value: result.p_value,
    })
}

fn convert_alternative(alternative: Alternative) -> YssSciAlternative {
    match alternative {
        Alternative::TwoSided => YssSciAlternative::TwoSided,
        Alternative::Greater => YssSciAlternative::Greater,
        Alternative::Less => YssSciAlternative::Less,
    }
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
    if input.betas.len() != coefficient_count
        || input.cov_beta.nrows() != coefficient_count
        || input.cov_beta.ncols() != coefficient_count
        || input.r_vec.len() != constraint_count
    {
        return Err(invalid_input(operation, SciInputViolation::ShapeMismatch));
    }
    if input
        .betas
        .iter()
        .chain(input.cov_beta.iter())
        .chain(input.r.iter())
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
    use ndarray::{Array1, Array2, arr1, arr2};

    fn input<'a>(
        betas: &'a Array1<f64>,
        cov_beta: &'a Array2<f64>,
        r: &'a Array2<f64>,
        r_vec: &'a Array1<f64>,
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
        let betas = arr1(&[1.0, 2.0]);
        let covariance = arr2(&[[0.1, 0.0], [0.0, 0.1]]);
        let constraint = arr2(&[[0.0, 1.0]]);
        let target = arr1(&[0.0]);

        assert_eq!(
            t_test(input(&betas, &covariance, &constraint, &target, 0)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::ParameterOutOfRange,
            }
        );

        let empty_constraints = Array2::zeros((0, 2));
        let empty_target = Array1::zeros(0);
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

        let short_betas = arr1(&[1.0]);
        assert_eq!(
            t_test(input(&short_betas, &covariance, &constraint, &target, 10,)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::ShapeMismatch,
            }
        );

        let short_covariance = arr2(&[[0.1]]);
        assert_eq!(
            t_test(input(&betas, &short_covariance, &constraint, &target, 10,)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::TTest,
                violation: SciInputViolation::ShapeMismatch,
            }
        );

        let long_target = arr1(&[0.0, 0.0]);
        assert_eq!(
            wald_test(input(&betas, &covariance, &constraint, &long_target, 10,)).unwrap_err(),
            SciError::InvalidInput {
                operation: SciOperationCode::WaldTest,
                violation: SciInputViolation::ShapeMismatch,
            }
        );

        let non_finite_betas = arr1(&[f64::NAN, 2.0]);
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
        let betas = arr1(&[1.0, 2.0]);
        let target = arr1(&[0.0]);
        let constraint = arr2(&[[0.0, 1.0]]);
        let zero_covariance = Array2::zeros((2, 2));
        assert_eq!(
            t_test(input(&betas, &zero_covariance, &constraint, &target, 10,)).unwrap_err(),
            SciError::ComputationFailed {
                operation: SciOperationCode::TTest,
            }
        );

        let constraints = arr2(&[[1.0, 0.0], [0.0, 1.0]]);
        let targets = arr1(&[0.0, 0.0]);
        let singular_covariance = arr2(&[[1.0, 1.0], [1.0, 1.0]]);
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
