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
fn automatic_hac_runtime_preserves_coefficient_inference_units() {
    use yss_sci_contract::regression::linear::{LinearRegressionMethod, LinearRegressionRequest};
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};

    let predictors = vec![
        (0..64)
            .map(|i| if i & 1 == 0 { 1.0 } else { -1.0 })
            .collect::<Vec<_>>(),
    ];
    let response = predictors[0]
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            1.0 + 0.5 * x
                + x * if (i & 48_usize).count_ones().is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                }
        })
        .collect::<Vec<_>>();
    let run = |unit| {
        linear_regression(
            LinearRegressionRequest {
                method: LinearRegressionMethod::Ols,
                response: response.iter().map(|y| y * unit).collect(),
                predictors: predictors.clone(),
                options: OlsOptions {
                    constant: true,
                    covariance: OlsCovariance::Hac {
                        kernel: "bartlett".into(),
                        bandwidth: None,
                    },
                },
            },
            &active_control(),
        )
        .unwrap()
    };
    let reference = run(1.0);
    for unit in [1e-12, 1e-151, 1e100] {
        let result = run(unit);
        assert_eq!(result.report.model_basic_info.covariance_type, "HAC");
        for (actual, expected) in result
            .report
            .coefficients
            .iter()
            .zip(&reference.report.coefficients)
        {
            assert!(
                (actual.std_err / unit - expected.std_err).abs() < 1e-10,
                "unit={unit}: normalized SE={} versus {}",
                actual.std_err / unit,
                expected.std_err
            );
            assert!((actual.t_value - expected.t_value).abs() < 1e-10);
            assert!((actual.p_value - expected.p_value).abs() < 1e-10);
        }
    }
    // Rational Bartlett(7) score products give variances 1/448 and 23/256.
    for (coefficient, variance) in reference
        .report
        .coefficients
        .iter()
        .zip([1.0 / 448.0, 23.0 / 256.0])
    {
        assert!((coefficient.std_err.powi(2) - variance).abs() < 1e-12);
    }
}

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
    let fit = yss_sci::regression::linear::fit::fit_ols(
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
