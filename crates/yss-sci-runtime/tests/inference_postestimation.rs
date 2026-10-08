//! Reference CR1 inference, adjusted means and shared binary marginal behavior.
use std::time::{Duration, Instant};
use yss_sci_contract::{
    MissingValuePolicy, StatisticalObservationMetadata,
    execution::*,
    regression::{
        OlsCovariance, OlsOptions,
        discrete::BinaryOptions,
        fit::{BinaryRegressionLink, FittedRegression},
        linear::*,
        postestimation::*,
    },
};
use yss_sci_runtime::{
    inference::cluster,
    regression::{
        discrete::fit_binary, linear::linear_regression, postestimation::adjusted_predictions,
    },
};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../yss-sci/tests/fixtures/postestimation_reference.json"
    ))
    .unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-7 * (1. + b.abs()), "{a} != {b}");
}
fn metadata(n: usize) -> StatisticalObservationMetadata {
    StatisticalObservationMetadata {
        original_observation_count: n,
        used_observation_count: n,
        dropped_null_count: 0,
        dropped_nan_count: 0,
        missing_value_policy: MissingValuePolicy::Reject,
    }
}

#[test]
fn cluster_input_failures_preserve_shapes_and_data_domain() {
    for (response, predictors, groups, violation) in [
        (
            vec![1., 2., 4.],
            vec![vec![0., 1., 2.]],
            vec![0, 1],
            ScientificInputViolation::ShapeMismatch,
        ),
        (
            vec![1., 2., 4.],
            vec![vec![0., 1., 2.]],
            vec![0, 0, 0],
            ScientificInputViolation::DataOutOfRange,
        ),
        (
            vec![1., 2.],
            vec![vec![0., 1.]],
            vec![0, 1],
            ScientificInputViolation::EmptyInput,
        ),
        (
            vec![1., 2., 4.],
            vec![],
            vec![0, 1, 0],
            ScientificInputViolation::EmptyInput,
        ),
        (
            vec![1., f64::NAN, 4.],
            vec![vec![0., 1., 2.]],
            vec![0, 1, 0],
            ScientificInputViolation::NonFiniteInput,
        ),
    ] {
        assert_eq!(
            cluster::fit(response, predictors, groups, true, &control()).unwrap_err(),
            ScientificComputationError::InvalidInput { violation },
        );
    }
}

#[test]
fn cluster_cr1_covariance_and_cluster_df_match_statsmodels_without_row_cap() {
    let f = fixture();
    let y: Vec<f64> = serde_json::from_value(f["response"].clone()).unwrap();
    let x: Vec<Vec<f64>> = serde_json::from_value(f["predictors"].clone()).unwrap();
    let g: Vec<usize> = serde_json::from_value(f["groups"].clone()).unwrap();
    let actual = cluster::fit(y.clone(), x.clone(), g.clone(), true, &control()).unwrap();
    assert_eq!(actual.observations, 640);
    assert_eq!(actual.clusters, 20);
    assert_eq!(actual.degrees_of_freedom, 19);
    for (j, c) in actual.coefficients.iter().enumerate() {
        close(c.estimate, f["cluster"]["beta"][j].as_f64().unwrap());
        close(c.p_value.unwrap(), f["cluster"]["p"][j].as_f64().unwrap());
        close(
            c.confidence_interval.unwrap()[0],
            f["cluster"]["ci"][j][0].as_f64().unwrap(),
        );
        for l in 0..3 {
            close(
                actual.covariance[j][l],
                f["cluster"]["covariance"][j][l].as_f64().unwrap(),
            );
        }
    }
    assert!(cluster::fit(y.clone(), x.clone(), vec![0; 640], true, &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        cluster::fit(y, x, g, true, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}

#[test]
fn adjusted_predictions_match_statsmodels_average_and_at_means_delta_intervals() {
    let f = fixture();
    let y: Vec<f64> = serde_json::from_value(f["response"].clone()).unwrap();
    let binary: Vec<f64> = serde_json::from_value(f["binary"].clone()).unwrap();
    let x: Vec<Vec<f64>> = serde_json::from_value(f["predictors"].clone()).unwrap();
    let linear = linear_regression(
        LinearRegressionRequest {
            response: y.clone(),
            predictors: x.clone(),
            options: OlsOptions::default(),
            method: LinearRegressionMethod::Ols,
        },
        &control(),
    )
    .unwrap();
    let robust = linear_regression(
        LinearRegressionRequest {
            response: y,
            predictors: x.clone(),
            options: OlsOptions {
                covariance: OlsCovariance::Hc3,
                ..Default::default()
            },
            method: LinearRegressionMethod::Ols,
        },
        &control(),
    )
    .unwrap();
    let logit = fit_binary(
        BinaryRegressionLink::Logit,
        binary.clone(),
        &x,
        BinaryOptions::default(),
        metadata(binary.len()),
    )
    .unwrap();
    let probit = fit_binary(
        BinaryRegressionLink::Probit,
        binary.clone(),
        &x,
        BinaryOptions::default(),
        metadata(binary.len()),
    )
    .unwrap();
    for case in f["predictions"].as_array().unwrap() {
        let model = match case["family"].as_str().unwrap() {
            "linear" => FittedRegression::Linear(&linear),
            "HC3" => FittedRegression::Linear(&robust),
            "logit" => FittedRegression::Binary(&logit),
            _ => FittedRegression::Binary(&probit),
        };
        let name = match model {
            FittedRegression::Linear(m) => &m.report.coefficients[1].variable,
            FittedRegression::Binary(m) => &m.parameter_names[1],
        };
        let mut options = PredictionOptions {
            evaluation: if case["evaluation"] == "average" {
                Evaluation::Average
            } else {
                Evaluation::AtMeans
            },
            at: if case["override"] == true {
                [(name.clone(), 0.75)].into()
            } else {
                Default::default()
            },
            confidence_level: 0.9,
        };
        let actual = adjusted_predictions(model, options.clone(), &control()).unwrap();
        close(actual.estimate, case["estimate"].as_f64().unwrap());
        close(actual.standard_error, case["se"].as_f64().unwrap());
        close(actual.lower, case["ci"][0].as_f64().unwrap());
        close(actual.upper, case["ci"][1].as_f64().unwrap());
        options.at.insert("unknown".into(), 1.);
        assert!(adjusted_predictions(model, options, &control()).is_err());
        let c = control();
        c.cancellation.cancel();
        assert!(matches!(
            adjusted_predictions(
                model,
                PredictionOptions {
                    evaluation: Evaluation::Average,
                    at: Default::default(),
                    confidence_level: 0.95
                },
                &c
            ),
            Err(ScientificComputationError::Cancelled)
        ));
    }
}
