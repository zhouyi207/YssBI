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
