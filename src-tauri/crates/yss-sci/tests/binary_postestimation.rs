//! Distinct regressions: reference derivative/delta-method values; classification
//! boundary/undefined rates and OR inference; asymptotic versus finite-sample tests.
use yss_sci::regression::discrete::{fit::fit_binary, postestimation::*};
use yss_sci_contract::regression::{discrete::*, fit::*};
use yss_sci_contract::{MissingValuePolicy, StatisticalObservationMetadata};
fn fixture_fit(link: BinaryRegressionLink) -> RegressionFit {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/binary_postestimation.json")).unwrap();
    let y: Vec<f64> = serde_json::from_value(f["response"].clone()).unwrap();
    let x: Vec<Vec<f64>> = serde_json::from_value(f["predictors"].clone()).unwrap();
    fit_binary(
        link,
        y.clone(),
        &x,
        BinaryOptions::default(),
        StatisticalObservationMetadata {
            original_observation_count: y.len(),
            used_observation_count: y.len(),
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: MissingValuePolicy::Reject,
        },
    )
    .unwrap()
}
fn control() -> yss_sci_contract::execution::ScientificExecutionControl {
    yss_sci_contract::execution::ScientificExecutionControl {
        cancellation: Default::default(),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
    }
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 2e-6, "{actual} != {expected}");
}
#[test]
fn binary_effects_match_statsmodels_for_both_links_and_all_transformations() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/binary_postestimation.json")).unwrap();
    for (i, link) in [BinaryRegressionLink::Logit, BinaryRegressionLink::Probit]
        .into_iter()
        .enumerate()
    {
        let fit = fixture_fit(link);
        let reference = &f["models"][i];
        for (a, b) in fit
            .coefficients
            .iter()
            .zip(reference["coefficients"].as_array().unwrap())
        {
            close(*a, b.as_f64().unwrap());
        }
        for (i, row) in fit
            .statistics
            .coefficient_statistics()
            .covariance
            .iter()
            .enumerate()
        {
            for (j, value) in row.iter().enumerate() {
                close(*value, reference["covariance"][i][j].as_f64().unwrap());
            }
        }
        for r in reference["effects"].as_array().unwrap() {
            let method = match r["method"].as_str().unwrap() {
                "dydx" => MarginalMethod::Dydx,
                "eyex" => MarginalMethod::Eyex,
                "eydx" => MarginalMethod::Eydx,
                _ => MarginalMethod::Dyex,
            };
            let at = if r["override"] == true {
                [("x1".into(), 0.75)].into()
            } else {
                Default::default()
            };
            let effects = marginal_effects(
                &fit,
                MarginalOptions {
                    evaluation: if r["evaluation"] == "mean" {
                        MarginalEvaluation::AtMeans
                    } else {
                        MarginalEvaluation::Average
                    },
                    method,
                    at,
                },
                &control(),
            )
            .unwrap();
            for (j, e) in effects.coefficients.iter().enumerate() {
                close(e.estimate, r["estimate"][j].as_f64().unwrap());
                close(e.standard_error, r["se"][j].as_f64().unwrap());
                close(e.p_value.unwrap(), r["p"][j].as_f64().unwrap());
                close(e.ci_lower, r["ci"][j][0].as_f64().unwrap());
                close(e.ci_upper, r["ci"][j][1].as_f64().unwrap());
            }
        }
    }
}
#[test]
fn odds_and_classification_keep_nulls_and_decision_boundary_semantics() {
    let mut fit = fixture_fit(BinaryRegressionLink::Logit);
    let odds = odds_ratios(&fit).unwrap();
    let inference = fit.statistics.coefficient_statistics();
    for (j, or) in odds.iter().enumerate() {
        close(or.estimate, fit.coefficients[j].exp());
        close(or.ci_lower, inference.confidence_interval_lower[j].exp());
        close(or.z_value.unwrap(), inference.statistic_values[j]);
    }
    assert!(odds_ratios(&fixture_fit(BinaryRegressionLink::Probit)).is_err());
    fit.fitted = vec![0.5, 0.7, 0.2, 0.4];
    fit.residuals = vec![0.5, -0.7, 0.8, -0.4];
    let c = classification(&fit, 0.5).unwrap();
    assert_eq!(
        (
            c.true_positive,
            c.false_positive,
            c.false_negative,
            c.true_negative
        ),
        (1, 1, 1, 1)
    );
    close(c.accuracy, 0.5);
    assert_eq!(
        classification(&fit, 0.).unwrap().negative_predictive_value,
        None
    );
    assert_eq!(
        classification(&fit, 1.).unwrap().positive_predictive_value,
        None
    );
    assert!(classification(&fit, f64::NAN).is_err());
    assert!(classification(&fit, 1.01).is_err());
    let mut fit = fixture_fit(BinaryRegressionLink::Logit);
    fit.coefficients[1] = 1000.;
    assert!(odds_ratios(&fit).is_err());
    let fit = fixture_fit(BinaryRegressionLink::Logit);
    let effect = marginal_effects(
        &fit,
        MarginalOptions {
            evaluation: MarginalEvaluation::AtMeans,
            method: MarginalMethod::Dyex,
            at: [("x1".into(), 0.)].into(),
        },
        &control(),
    )
    .unwrap();
    close(effect.coefficients[0].estimate, 0.);
    close(effect.coefficients[0].standard_error, 0.);
    assert_eq!(effect.coefficients[0].z_value, None);
    let mut malformed = fit.clone();
    if let RegressionStatistics::Binary { coefficients, .. } = &mut malformed.statistics {
        coefficients.standard_errors.clear();
    }
    assert!(odds_ratios(&malformed).is_err());
    let mut negative = fit.clone();
    if let RegressionStatistics::Binary { coefficients, .. } = &mut negative.statistics {
        for (i, row) in coefficients.covariance.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = if i == j { -1e-20 } else { 0.0 };
            }
        }
    }
    assert!(
        marginal_effects(
            &negative,
            MarginalOptions {
                evaluation: MarginalEvaluation::Average,
                method: MarginalMethod::Dydx,
                at: Default::default()
            },
            &control()
        )
        .is_err()
    );
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert_eq!(
        marginal_effects(
            &fit,
            MarginalOptions {
                evaluation: MarginalEvaluation::Average,
                method: MarginalMethod::Dydx,
                at: Default::default()
            },
            &cancelled
        )
        .unwrap_err(),
        yss_sci_contract::execution::ScientificComputationError::Cancelled
    );
    let tail = marginal_effects(
        &fit,
        MarginalOptions {
            evaluation: MarginalEvaluation::AtMeans,
            method: MarginalMethod::Eydx,
            at: [("x1".into(), -2000.0)].into(),
        },
        &control(),
    )
    .unwrap();
    close(tail.coefficients[0].estimate, fit.coefficients[1]);
    assert!(tail.coefficients[0].standard_error.is_finite());
}
#[test]
fn coefficient_restrictions_use_z_and_chisquare_for_likelihood_models() {
    use yss_sci::hypothesis::linear_hypothesis::{
        run_asymptotic_hypothesis_test, run_hypothesis_test,
    };
    use yss_sci_contract::hypothesis::HypothesisTestInput;
    let input = |hypothesis: &str| HypothesisTestInput {
        betas: vec![1., 2.],
        cov_beta: vec![vec![1., 0.], vec![0., 1.]],
        df_residual: 8,
        param_names: vec!["a".into(), "b".into()],
        hypothesis: hypothesis.into(),
    };
    let z = run_asymptotic_hypothesis_test(input("b = 0")).unwrap();
    assert_eq!(z.test_type, "z");
    close(z.stat, 2.);
    close(z.p_value, 0.0455002638963584);
    assert!(run_hypothesis_test(input("b = 0")).unwrap().p_value > z.p_value);
    let chi = run_asymptotic_hypothesis_test(input("a = 0, b = 0")).unwrap();
    assert_eq!(chi.test_type, "chi2");
    close(chi.stat, 5.);
    close(chi.p_value, (-2.5f64).exp());
    assert!(run_asymptotic_hypothesis_test(input("a = 0, 2*a = 0")).is_err());
}
