use std::time::{Duration, Instant};
use yss_sci_contract::execution::{ScientificCancellationToken, ScientificExecutionControl};

fn active_control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}

use super::linear_regression;

#[test]
fn weighted_admission_classifies_observation_failures() {
    use yss_sci_contract::execution::{ScientificComputationError, ScientificInputViolation};
    use yss_sci_contract::regression::linear::{LinearRegressionMethod, LinearRegressionRequest};
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};

    let identity = (0..4)
        .map(|i| (0..4).map(|j| f64::from(i == j)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut nonfinite = identity.clone();
    nonfinite[0][0] = f64::NAN;
    let mut asymmetric = identity.clone();
    asymmetric[0][1] = 0.5;
    for (method, covariance, violation) in [
        (
            LinearRegressionMethod::Wls {
                weights: vec![1.; 3],
            },
            OlsCovariance::NonRobust,
            ScientificInputViolation::ShapeMismatch,
        ),
        (
            LinearRegressionMethod::Wls {
                weights: vec![1., f64::NAN, 1., 1.],
            },
            OlsCovariance::NonRobust,
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            LinearRegressionMethod::Wls {
                weights: vec![1., 0., 1., 1.],
            },
            OlsCovariance::NonRobust,
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            LinearRegressionMethod::Gls {
                sigma: vec![vec![1.; 3]; 4],
            },
            OlsCovariance::NonRobust,
            ScientificInputViolation::ShapeMismatch,
        ),
        (
            LinearRegressionMethod::Gls { sigma: nonfinite },
            OlsCovariance::NonRobust,
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            LinearRegressionMethod::Gls { sigma: asymmetric },
            OlsCovariance::NonRobust,
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            LinearRegressionMethod::Gls { sigma: identity },
            OlsCovariance::Hc3,
            ScientificInputViolation::ParameterOutOfRange,
        ),
    ] {
        let error = linear_regression(
            LinearRegressionRequest {
                response: vec![1., 2., 4., 3.],
                predictors: vec![vec![0., 1., 2., 3.]],
                options: OlsOptions {
                    constant: true,
                    covariance,
                },
                method,
            },
            &active_control(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            ScientificComputationError::InvalidInput { violation }
        );
    }
}

#[test]
fn direct_ols_admission_classifies_observation_failures() {
    use yss_sci_contract::execution::ScientificInputViolation;
    use yss_sci_contract::{
        MissingValuePolicy, SciError, SciOperationCode, StatisticalObservationMetadata,
    };

    for (response, predictors, violation) in [
        (
            vec![1., 2.],
            vec![vec![0., 1.]],
            ScientificInputViolation::EmptyInput,
        ),
        (
            vec![1., f64::NAN, 3.],
            vec![vec![0., 1., 2.]],
            ScientificInputViolation::NonFiniteInput,
        ),
        (
            vec![1., 2., 3.],
            vec![vec![0., f64::INFINITY, 2.]],
            ScientificInputViolation::NonFiniteInput,
        ),
    ] {
        let observations = response.len();
        let error = super::fit_ols(
            response,
            &predictors,
            Default::default(),
            StatisticalObservationMetadata {
                original_observation_count: observations,
                used_observation_count: observations,
                dropped_null_count: 0,
                dropped_nan_count: 0,
                missing_value_policy: MissingValuePolicy::Reject,
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            SciError::InvalidInput {
                operation: SciOperationCode::Regression,
                violation
            }
        );
    }
}

#[test]
fn shared_ols_options_reach_the_model_and_typed_report() {
    use yss_sci_contract::regression::linear::{LinearRegressionMethod, LinearRegressionRequest};
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
    let response = vec![1.1, 2.2, 2.8, 4.1, 5.3, 5.7, 7.2, 8.4];
    let predictors = vec![(1..=8).map(f64::from).collect::<Vec<_>>()];
    let options = OlsOptions {
        constant: false,
        covariance: OlsCovariance::Hc3,
    };
    let result = linear_regression(
        LinearRegressionRequest {
            method: LinearRegressionMethod::Ols,
            response: response.clone(),
            predictors: predictors.clone(),
            options: options.clone(),
        },
        &active_control(),
    )
    .unwrap();
    let numerical = yss_sci::regression::linear::fit::fit_ols(
        response.clone(),
        &predictors,
        options,
        yss_sci_contract::StatisticalObservationMetadata {
            original_observation_count: response.len(),
            used_observation_count: response.len(),
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
        },
    )
    .unwrap();
    let yss_sci_contract::regression::fit::RegressionStatistics::Linear { model, .. } =
        &numerical.statistics
    else {
        panic!("OLS must produce linear regression statistics");
    };
    assert_eq!(result.coefficients, numerical.coefficients);
    assert_eq!(result.fitted, numerical.fitted);
    assert_eq!(result.residuals, numerical.residuals);
    assert_eq!(result.design, predictors);
    assert_eq!(result.report.model_basic_info.covariance_type, "HC3");
    assert_eq!(
        result.report.model_basic_info.df_residual,
        model.df_residual
    );
    assert_eq!(result.report.coefficients.len(), 1);
    assert_eq!(result.report.coefficients[0].variable, "x1");
    let nonrobust = linear_regression(
        LinearRegressionRequest {
            method: LinearRegressionMethod::Ols,
            response,
            predictors,
            options: OlsOptions {
                constant: false,
                covariance: OlsCovariance::NonRobust,
            },
        },
        &active_control(),
    )
    .unwrap();
    assert_ne!(
        result.report.coefficients[0].std_err,
        nonrobust.report.coefficients[0].std_err
    );
}

#[test]
fn report_confidence_limits_preserve_scientific_precision_for_small_units() {
    let response = [1.1, 2.2, 2.8, 4.1, 5.3, 5.7, 7.2, 8.4]
        .into_iter()
        .map(|value| value * 1e-13)
        .collect::<Vec<_>>();
    let predictors = vec![(1..=8).map(f64::from).collect::<Vec<_>>()];
    let fit = super::fit_ols(
        response.clone(),
        &predictors,
        Default::default(),
        yss_sci_contract::StatisticalObservationMetadata {
            original_observation_count: response.len(),
            used_observation_count: response.len(),
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
        },
    )
    .unwrap();
    let summary = crate::regression::report::linear_regression_report(&fit).unwrap();
    let inference = fit.statistics.coefficient_statistics();
    for (index, row) in summary.coefficients.iter().enumerate() {
        assert_eq!(
            row.ci_lower.to_bits(),
            inference.confidence_interval_lower[index].to_bits()
        );
        assert_eq!(
            row.ci_upper.to_bits(),
            inference.confidence_interval_upper[index].to_bits()
        );
    }
}
