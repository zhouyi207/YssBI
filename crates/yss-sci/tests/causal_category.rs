use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci::causal::{designs, econometrics, treatment};
use yss_sci_contract::{causal::models::*, execution::*, regression::models::IterationOptions};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/causal_category_reference.json")).unwrap()
}
fn vector(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
fn matrix(value: &Value) -> Vec<Vec<f64>> {
    serde_json::from_value(value.clone()).unwrap()
}
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}
fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * (1.0 + expected.abs()),
        "{actual} != {expected}"
    );
}
fn coefficients(
    actual: &[yss_sci_contract::regression::models::RegressionCoefficient],
    expected: &Value,
    tolerance: f64,
) {
    let expected = vector(expected);
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        close(a.estimate, e, tolerance);
    }
}
fn covariance(actual: &[Vec<f64>], expected: &Value, tolerance: f64) {
    let expected = matrix(expected);
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.len(), e.len());
        for (&a, e) in a.iter().zip(e) {
            close(a, e, tolerance);
        }
    }
}
fn options(method: TreatmentMethod) -> TreatmentOptions {
    TreatmentOptions {
        method,
        overlap: 1e-6,
        caliper: 1.0,
        iteration: IterationOptions::default(),
        bootstrap: BootstrapOptions {
            replications: 0,
            seed: 42,
        },
    }
}

#[test]
fn treatment_estimands_match_independent_references_and_bootstrap_refits() {
    let f = fixture();
    let d = &f["treatment"];
    let y = vector(&d["response"]);
    let t = vector(&d["treatment"]);
    let x = matrix(&d["predictors"]);
    for (method, key) in [
        (TreatmentMethod::Matching, "matching"),
        (TreatmentMethod::Ipw, "ipw"),
        (TreatmentMethod::RegressionAdjustment, "ra"),
        (TreatmentMethod::Aipw, "aipw"),
    ] {
        let r = treatment::estimate(&y, &t, &x, options(method), &control()).unwrap();
        close(r.ate.estimate, d["cases"][key][0].as_f64().unwrap(), 1e-8);
        close(r.att.estimate, d["cases"][key][1].as_f64().unwrap(), 1e-8);
        assert!(r.ate.standard_error.is_none());
        if let Some(e) = r.propensity_scores {
            for (a, b) in e.iter().zip(vector(&d["propensity"])) {
                close(*a, b, 1e-8);
            }
        }
        if method != TreatmentMethod::Matching {
            let opts = TreatmentOptions {
                bootstrap: BootstrapOptions {
                    replications: 24,
                    seed: 17,
                },
                ..options(method)
            };
            let boot = treatment::estimate(&y, &t, &x, opts, &control()).unwrap();
            let transformed = y.iter().map(|v| 3.0 + 2.0 * v).collect::<Vec<_>>();
            let scaled = treatment::estimate(&transformed, &t, &x, opts, &control()).unwrap();
            close(boot.ate.estimate, r.ate.estimate, 1e-12);
            assert!(boot.ate.standard_error.unwrap() > 0.0);
            close(
                scaled.ate.standard_error.unwrap(),
                2.0 * boot.ate.standard_error.unwrap(),
                1e-8,
            );
            close(scaled.att.estimate, 2.0 * boot.att.estimate, 1e-8);
            assert_eq!(boot.bootstrap_replications, 24);
        }
    }
    // Exact score ties average all opposite-treatment outcomes, independent of row order.
    let r = treatment::estimate(
        &[1., 3., 5., 9.],
        &[0., 0., 1., 1.],
        &[],
        options(TreatmentMethod::Matching),
        &control(),
    )
    .unwrap();
    assert_eq!(r.match_counts.unwrap(), [2, 2, 2, 2]);
    close(r.ate.estimate, 5.0, 1e-12);
}

#[test]
fn treatment_response_units_keep_ipw_effects_finite() {
    let f = fixture();
    let d = &f["treatment"];
    let scale = 1e306;
    let response = vector(&d["response"])
        .into_iter()
        .map(|value| value * scale)
        .collect::<Vec<_>>();
    assert!(response.iter().all(|value| value.is_finite()));
    let result = treatment::estimate(
        &response,
        &vector(&d["treatment"]),
        &matrix(&d["predictors"]),
        options(TreatmentMethod::Ipw),
        &control(),
    )
    .expect("normalized IPW means and effects are finite in these response units");
    assert!(result.ate.estimate.is_finite() && result.att.estimate.is_finite());
    close(
        result.ate.estimate / scale,
        d["cases"]["ipw"][0].as_f64().unwrap(),
        1e-8,
    );
    close(
        result.att.estimate / scale,
        d["cases"]["ipw"][1].as_f64().unwrap(),
        1e-8,
    );
    assert!(result.ate.standard_error.is_none() && result.att.standard_error.is_none());
}

#[test]
fn treatment_response_units_keep_bootstrap_inference_finite() {
    let f = fixture();
    let d = &f["treatment"];
    let response = vector(&d["response"]);
    let treated = vector(&d["treatment"]);
    let predictors = matrix(&d["predictors"]);
    let scale = 5e154;
    let scaled_response = response
        .iter()
        .map(|value| value * scale)
        .collect::<Vec<_>>();
    for method in [
        TreatmentMethod::Ipw,
        TreatmentMethod::RegressionAdjustment,
        TreatmentMethod::Aipw,
    ] {
        let options = TreatmentOptions {
            bootstrap: BootstrapOptions {
                replications: 24,
                seed: 17,
            },
            ..options(method)
        };
        let baseline =
            treatment::estimate(&response, &treated, &predictors, options, &control()).unwrap();
        for effect in [&baseline.ate, &baseline.att] {
            let expected_se = effect.standard_error.unwrap() * scale;
            assert!(expected_se.is_finite() && (expected_se * expected_se).is_finite());
        }
        let scaled =
            treatment::estimate(&scaled_response, &treated, &predictors, options, &control())
                .expect("bootstrap coefficient covariance is finite in these response units");
        for (actual, expected) in [(&scaled.ate, &baseline.ate), (&scaled.att, &baseline.att)] {
            close(actual.estimate / scale, expected.estimate, 1e-8);
            close(
                actual.standard_error.unwrap() / scale,
                expected.standard_error.unwrap(),
                1e-8,
            );
            close(actual.statistic.unwrap(), expected.statistic.unwrap(), 1e-8);
            close(actual.p_value.unwrap(), expected.p_value.unwrap(), 1e-8);
            for (bound, expected) in actual
                .confidence_interval
                .unwrap()
                .into_iter()
                .zip(expected.confidence_interval.unwrap())
            {
                assert!(bound.is_finite());
                close(bound / scale, expected, 1e-8);
            }
        }
        assert_eq!(scaled.bootstrap_replications, 24);
        assert_eq!(scaled.inference, baseline.inference);

        let small_scale = 1e-160;
        let small_response = response
            .iter()
            .map(|value| value * small_scale)
            .collect::<Vec<_>>();
        let small =
            treatment::estimate(&small_response, &treated, &predictors, options, &control())
                .expect("representable subnormal bootstrap covariance must retain inference");
        for (actual, expected) in [(&small.ate, &baseline.ate), (&small.att, &baseline.att)] {
            let standard_error = actual.standard_error.unwrap();
            let expected_variance = (expected.standard_error.unwrap() * small_scale).powi(2);
            assert!(expected_variance > 0.0 && expected_variance.is_subnormal());
            assert!(standard_error.is_finite() && standard_error > 0.0);
            // At these units covariance is subnormal; compare its representable
            // values within one ULP rather than demanding normal precision.
            assert!(
                standard_error
                    .powi(2)
                    .to_bits()
                    .abs_diff(expected_variance.to_bits())
                    <= 1,
                "{method:?}: {} != {expected_variance}",
                standard_error.powi(2)
            );
            close(actual.estimate / small_scale, expected.estimate, 1e-8);
            assert!(actual.statistic.unwrap().is_finite());
            assert!(actual.p_value.unwrap().is_finite());
            assert!(
                actual
                    .confidence_interval
                    .unwrap()
                    .iter()
                    .all(|bound| bound.is_finite())
            );
        }
    }
}

#[test]
fn gmm_matches_statsmodels_robust_covariance_and_hansen_test() {
    let f = fixture();
    let d = &f["gmm"];
    let y = vector(&d["response"]);
    let x = matrix(&d["predictors"]);
    let z = matrix(&d["instruments"]);
    for case in d["cases"].as_array().unwrap() {
        let two = case["steps"] == 2;
        let r = econometrics::gmm(
            &y,
            &x,
            &z,
            GmmOptions {
                constant: true,
                two_step: two,
            },
            &control(),
        )
        .unwrap();
        coefficients(&r.coefficients, &case["coefficients"], 1e-8);
        covariance(&r.covariance, &case["covariance"], 1e-8);
        if two {
            let j = r.hansen_j.unwrap();
            close(j.statistic, case["j"].as_f64().unwrap(), 1e-8);
            assert_eq!(j.degrees_of_freedom, 1);
        } else {
            assert!(r.hansen_j.is_none());
        }
    }
    let r = econometrics::gmm(
        &y,
        &x,
        &z[..2],
        GmmOptions {
            constant: true,
            two_step: true,
        },
        &control(),
    )
    .unwrap();
    assert!(r.hansen_j.is_none());
}

fn assert_gmm_response_units(scale: f64, predictor_scale: f64) {
    let f = fixture();
    let d = &f["gmm"];
    let y = vector(&d["response"]);
    let x = matrix(&d["predictors"]);
    let z = matrix(&d["instruments"]);
    let scaled = y.iter().map(|value| value * scale).collect::<Vec<_>>();
    let scaled_predictors = x
        .iter()
        .map(|column| column.iter().map(|value| value * predictor_scale).collect())
        .collect::<Vec<Vec<f64>>>();
    for case in d["cases"].as_array().unwrap() {
        let two_step = case["steps"] == 2;
        let result = econometrics::gmm(
            &scaled,
            &scaled_predictors,
            &z,
            GmmOptions {
                constant: true,
                two_step,
            },
            &control(),
        )
        .expect("representable GMM inference must retain its response units");
        let beta = vector(&case["coefficients"]);
        let covariance = matrix(&case["covariance"]);
        let units = |j: usize| {
            if j == 0 {
                scale
            } else {
                scale / predictor_scale
            }
        };
        for (j, coefficient) in result.coefficients.iter().enumerate() {
            close(coefficient.estimate / units(j), beta[j], 1e-8);
            let standard_error = covariance[j][j].sqrt();
            close(
                coefficient.standard_error.unwrap() / units(j),
                standard_error,
                1e-8,
            );
            close(
                coefficient.statistic.unwrap(),
                beta[j] / standard_error,
                1e-8,
            );
            for (k, &value) in result.covariance[j].iter().enumerate() {
                assert!(value.is_finite());
                close(value / units(j) / units(k), covariance[j][k], 1e-8);
            }
        }
        let predicted = (0..y.len())
            .map(|i| beta[0] + (0..x.len()).map(|j| x[j][i] * beta[j + 1]).sum::<f64>())
            .collect::<Vec<_>>();
        for (i, (&prediction, &residual)) in result.fitted.iter().zip(&result.residuals).enumerate()
        {
            close(prediction / scale, predicted[i], 1e-8);
            close(residual / scale, y[i] - predicted[i], 1e-8);
        }
        for (j, &moment) in result.moments.iter().enumerate() {
            let expected = (0..y.len())
                .map(|i| {
                    let instrument = if j == 0 { 1.0 } else { z[j - 1][i] };
                    instrument * (y[i] - predicted[i]) / y.len() as f64
                })
                .sum::<f64>();
            close(moment / scale, expected, 1e-8);
        }
        if two_step {
            let test = result.hansen_j.unwrap();
            close(test.statistic, case["j"].as_f64().unwrap(), 1e-8);
            assert_eq!(test.degrees_of_freedom, 1);
            assert!(test.p_value.is_finite());
        } else {
            assert!(result.hansen_j.is_none());
        }
    }
}

#[test]
fn gmm_large_response_units_keep_finite_inference() {
    assert_gmm_response_units(1e154, 1.0);
}

#[test]
fn gmm_small_response_units_keep_two_step_weights() {
    assert_gmm_response_units(1e-155, 1e-160);
}

#[test]
fn discontinuity_and_group_interactions_match_hc3_inference() {
    let f = fixture();
    let d = &f["rdd"];
    for case in d["cases"].as_array().unwrap() {
        let r = designs::rdd(
            &vector(&d["response"]),
            &vector(&d["running"]),
            RddOptions {
                cutoff: 0.0,
                bandwidth: 1.0,
                triangular: case["triangular"].as_bool().unwrap(),
            },
            &control(),
        )
        .unwrap();
        coefficients(&r.coefficients, &case["coefficients"], 1e-8);
        covariance(&r.covariance, &case["covariance"], 1e-8);
        assert_eq!(r.left_observations, r.right_observations);
        assert!(r.rows[0] > 1);
    }
    let d = &f["treatment"];
    let h = &f["heterogeneity"];
    let groups = serde_json::from_value::<Vec<usize>>(h["groups"].clone()).unwrap();
    let r = designs::heterogeneity(
        &vector(&d["response"]),
        &vector(&d["treatment"]),
        &groups,
        &matrix(&d["predictors"]),
        &control(),
    )
    .unwrap();
    coefficients(&r.coefficients, &h["coefficients"], 1e-8);
    covariance(&r.covariance, &h["covariance"], 1e-8);
    close(
        r.equality_test.statistic,
        h["statistic"].as_f64().unwrap(),
        1e-8,
    );
    assert_eq!(r.equality_test.degrees_of_freedom, 1);
}

#[test]
fn causal_hc3_keeps_resolved_weighted_leverage_below_one() {
    let edge = 1.0 - 2.0_f64.powi(-44);
    let running = [-0.25, -0.5, -edge, 0.25, 0.5, edge];
    let response = running
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            if x < 0.0 {
                1.0 + 2.0 * x + if i == 2 { 1.0 } else { 0.0 }
            } else {
                4.0 + x - if i == 5 { 2.0 } else { 0.0 }
            }
        })
        .collect::<Vec<_>>();
    let result = designs::rdd(
        &response,
        &running,
        RddOptions {
            cutoff: 0.0,
            bandwidth: 1.0,
            triangular: true,
        },
        &control(),
    )
    .expect("positive resolved HC3 remainders must not have an absolute cutoff");
    for (coefficient, expected) in result.coefficients.iter().zip([1.0, 3.0, 2.0, -1.0]) {
        close(coefficient.estimate, expected, 1e-8);
        assert!(coefficient.standard_error.unwrap().is_finite());
        assert!(coefficient.standard_error.unwrap() > 0.0);
    }
    assert!(result.covariance.iter().flatten().all(|v| v.is_finite()));
    assert_eq!(result.left_observations, 3);
    assert_eq!(result.right_observations, 3);
}

#[test]
fn causal_hc3_keeps_finite_inference_for_large_response_units() {
    let f = fixture();
    let scale = 1e154;
    let check = |actual: &[Vec<f64>], expected: &Value| {
        let expected = matrix(expected);
        for (row, reference) in actual.iter().zip(expected) {
            for (&value, expected) in row.iter().zip(reference) {
                assert!(value.is_finite());
                close(value / scale / scale, expected, 1e-8);
            }
        }
    };
    let d = &f["rdd"];
    let response = vector(&d["response"])
        .into_iter()
        .map(|v| v * scale)
        .collect::<Vec<_>>();
    for case in d["cases"].as_array().unwrap() {
        let result = designs::rdd(
            &response,
            &vector(&d["running"]),
            RddOptions {
                cutoff: 0.0,
                bandwidth: 1.0,
                triangular: case["triangular"].as_bool().unwrap(),
            },
            &control(),
        )
        .expect("RDD HC3 coefficient covariance is finite in these units");
        check(&result.covariance, &case["covariance"]);
    }
    let d = &f["treatment"];
    let h = &f["heterogeneity"];
    let response = vector(&d["response"])
        .into_iter()
        .map(|v| v * scale)
        .collect::<Vec<_>>();
    let result = designs::heterogeneity(
        &response,
        &vector(&d["treatment"]),
        &serde_json::from_value::<Vec<usize>>(h["groups"].clone()).unwrap(),
        &matrix(&d["predictors"]),
        &control(),
    )
    .expect("unweighted HC3 coefficient covariance is finite in these units");
    check(&result.covariance, &h["covariance"]);
    close(
        result.equality_test.statistic,
        h["statistic"].as_f64().unwrap(),
        1e-8,
    );
}

fn check_heckman_response_units(scale: f64) {
    let f = fixture();
    let d = &f["heckman"];
    let response = serde_json::from_value::<Vec<Option<f64>>>(d["response"].clone()).unwrap();
    let selected = vector(&d["selected"]);
    let predictors = matrix(&d["predictors"]);
    let selection_predictors = matrix(&d["selection_predictors"]);
    let options = HeckmanOptions {
        iteration: IterationOptions::default(),
        bootstrap: BootstrapOptions {
            replications: 0,
            seed: 37,
        },
    };
    let baseline = econometrics::heckman(
        &response,
        &selected,
        &predictors,
        &selection_predictors,
        options,
        &control(),
    )
    .unwrap();
    let scaled_response = response
        .iter()
        .map(|value| value.map(|value| value * scale))
        .collect::<Vec<_>>();
    let result = econometrics::heckman(
        &scaled_response,
        &selected,
        &predictors,
        &selection_predictors,
        options,
        &control(),
    )
    .expect("reported Heckman sigma and rho are finite in these response units");
    for (actual, expected) in result
        .outcome_coefficients
        .iter()
        .zip(vector(&d["coefficients"]))
    {
        assert!(actual.estimate.is_finite());
        close(actual.estimate / scale, expected, 1e-7);
        assert!(actual.standard_error.is_none());
    }
    assert!(result.sigma.is_finite() && result.sigma > 0.0);
    close(result.sigma / scale, d["sigma"].as_f64().unwrap(), 1e-7);
    close(result.rho, d["rho"].as_f64().unwrap(), 1e-7);
    assert_eq!(result.selected_rows, baseline.selected_rows);
    assert_eq!(
        result.selection_probabilities,
        baseline.selection_probabilities
    );
    assert_eq!(result.inverse_mills, baseline.inverse_mills);
    for (actual, expected) in result.fitted_selected.iter().zip(baseline.fitted_selected) {
        assert!(actual.is_finite());
        close(actual / scale, expected, 1e-7);
    }
    for (actual, expected) in result
        .residuals_selected
        .iter()
        .zip(baseline.residuals_selected)
    {
        assert!(actual.is_finite());
        close(actual / scale, expected, 1e-7);
    }
    assert!(result.outcome_covariance.is_none());
}

#[test]
fn heckman_response_units_keep_large_sigma_finite() {
    check_heckman_response_units(1e200);
}

#[test]
fn heckman_response_units_keep_small_sigma_identified() {
    check_heckman_response_units(1e-200);
}

#[test]
fn selection_and_frontier_match_probit_and_likelihood_references() {
    let f = fixture();
    let d = &f["heckman"];
    let y = serde_json::from_value::<Vec<Option<f64>>>(d["response"].clone()).unwrap();
    let r = econometrics::heckman(
        &y,
        &vector(&d["selected"]),
        &matrix(&d["predictors"]),
        &matrix(&d["selection_predictors"]),
        HeckmanOptions {
            iteration: IterationOptions::default(),
            bootstrap: BootstrapOptions {
                replications: 16,
                seed: 37,
            },
        },
        &control(),
    )
    .unwrap();
    coefficients(&r.outcome_coefficients, &d["coefficients"], 1e-7);
    coefficients(
        &r.selection_coefficients,
        &d["selection_coefficients"],
        1e-7,
    );
    close(r.sigma, d["sigma"].as_f64().unwrap(), 1e-7);
    close(r.rho, d["rho"].as_f64().unwrap(), 1e-7);
    assert!(r.outcome_covariance.is_some());
    assert!(r.outcome_coefficients[1].standard_error.unwrap() > 0.0);
    assert_eq!(r.selected_rows.len(), r.fitted_selected.len());
    let d = &f["frontier"];
    let r = econometrics::frontier(
        &vector(&d["response"]),
        &matrix(&d["predictors"]),
        FrontierOptions {
            constant: true,
            cost: false,
            iteration: IterationOptions::default(),
        },
        &control(),
    )
    .unwrap();
    coefficients(&r.coefficients, &d["coefficients"], 3e-5);
    covariance(&r.covariance, &d["covariance"], 3e-5);
    close(r.sigma_u, d["sigma_u"].as_f64().unwrap(), 3e-5);
    close(r.sigma_v, d["sigma_v"].as_f64().unwrap(), 3e-5);
    close(
        r.log_likelihood,
        d["log_likelihood"].as_f64().unwrap(),
        1e-7,
    );
    for (&a, b) in r.efficiency.iter().zip(vector(&d["efficiency"])) {
        close(a, b, 3e-5);
        assert!(a > 0.0 && a <= 1.0);
    }
    let costs = vector(&d["response"])
        .iter()
        .map(|v| -v)
        .collect::<Vec<_>>();
    let c = econometrics::frontier(
        &costs,
        &matrix(&d["predictors"]),
        FrontierOptions {
            constant: true,
            cost: true,
            iteration: IterationOptions::default(),
        },
        &control(),
    )
    .unwrap();
    for (a, b) in r.coefficients.iter().zip(c.coefficients) {
        close(a.estimate, -b.estimate, 3e-5);
    }
}

#[test]
fn sur_response_units_keep_large_residual_covariance_finite() {
    check_sur_response_units([5e153, 5e153]);
}

#[test]
fn sur_response_units_keep_subnormal_equation_covariance_identified() {
    let result = check_sur_response_units([1e-155, 1e153]);
    assert!(result.error_covariance[0][0].is_subnormal());
    assert!(result.error_covariance[0][0] > 0.0);
    for j in 0..result.equations[0].coefficients.len() {
        assert!(result.coefficient_covariance[j][j].is_subnormal());
        assert!(result.coefficient_covariance[j][j] > 0.0);
    }
}

fn check_sur_response_units(scales: [f64; 2]) -> SurResult {
    let f = fixture();
    let d = &f["sur"];
    let responses = matrix(&d["responses"]);
    let predictors = matrix(&d["predictors"]);
    let selections = [vec![0, 1], vec![0, 2]];
    let baseline =
        econometrics::sur(&responses, &predictors, &selections, true, &control()).unwrap();
    let responses = responses
        .iter()
        .zip(scales)
        .map(|(values, scale)| values.iter().map(|value| value * scale).collect())
        .collect::<Vec<_>>();
    let result = econometrics::sur(&responses, &predictors, &selections, true, &control())
        .expect("SUR covariance and inference are finite in these equation units");
    let reference_coefficients = vector(&d["coefficients"]);
    let reference_covariance = matrix(&d["covariance"]);
    assert_eq!(result.observations, baseline.observations);
    assert_eq!(result.equations.len(), baseline.equations.len());
    let mut offset = 0;
    let mut coefficient_scales = Vec::new();
    for ((equation, baseline), scale) in
        result.equations.iter().zip(&baseline.equations).zip(scales)
    {
        assert_eq!(equation.predictors, baseline.predictors);
        assert_eq!(equation.coefficients.len(), baseline.coefficients.len());
        for (j, (actual, baseline)) in equation
            .coefficients
            .iter()
            .zip(&baseline.coefficients)
            .enumerate()
        {
            let axis = offset + j;
            let standard_error = reference_covariance[axis][axis].sqrt();
            assert_eq!(actual.term, baseline.term);
            assert!(actual.estimate.is_finite());
            close(actual.estimate / scale, reference_coefficients[axis], 1e-8);
            close(actual.standard_error.unwrap() / scale, standard_error, 1e-8);
            close(
                actual.statistic.unwrap(),
                reference_coefficients[axis] / standard_error,
                1e-8,
            );
            close(actual.p_value.unwrap(), baseline.p_value.unwrap(), 1e-8);
            for (actual, baseline) in actual
                .confidence_interval
                .unwrap()
                .into_iter()
                .zip(baseline.confidence_interval.unwrap())
            {
                close(actual / scale, baseline, 1e-8);
            }
            coefficient_scales.push(scale);
        }
        offset += equation.coefficients.len();
        assert_eq!(equation.fitted.len(), baseline.fitted.len());
        assert_eq!(equation.residuals.len(), baseline.residuals.len());
        for (actual, baseline) in equation
            .fitted
            .iter()
            .chain(&equation.residuals)
            .zip(baseline.fitted.iter().chain(&baseline.residuals))
        {
            assert!(actual.is_finite());
            close(actual / scale, *baseline, 1e-8);
        }
    }
    let check_covariance = |actual: &[Vec<f64>], reference: &[Vec<f64>], scales: &[f64]| {
        assert_eq!(actual.len(), reference.len());
        for (i, (actual, reference)) in actual.iter().zip(reference).enumerate() {
            assert_eq!(actual.len(), reference.len());
            for (j, (&actual, &reference)) in actual.iter().zip(reference).enumerate() {
                assert!(actual.is_finite());
                close(
                    actual / scales[i].max(scales[j]) / scales[i].min(scales[j]),
                    reference,
                    1e-8,
                );
            }
        }
    };
    check_covariance(
        &result.error_covariance,
        &matrix(&d["error_covariance"]),
        &scales,
    );
    check_covariance(
        &result.coefficient_covariance,
        &reference_covariance,
        &coefficient_scales,
    );
    result
}

#[test]
fn sur_equation_designs_and_synthetic_preperiod_fit_match_references() {
    let f = fixture();
    let d = &f["sur"];
    let r = econometrics::sur(
        &matrix(&d["responses"]),
        &matrix(&d["predictors"]),
        &[vec![0, 1], vec![0, 2]],
        true,
        &control(),
    )
    .unwrap();
    let beta = r
        .equations
        .into_iter()
        .flat_map(|e| e.coefficients)
        .collect::<Vec<_>>();
    coefficients(&beta, &d["coefficients"], 1e-8);
    covariance(&r.coefficient_covariance, &d["covariance"], 1e-8);
    covariance(&r.error_covariance, &d["error_covariance"], 1e-8);
    let d = &f["synthetic"];
    let mut y = vector(&d["response"]);
    let mut donors = matrix(&d["donors"]);
    let opts = SyntheticControlOptions {
        pre_periods: 20,
        iteration: IterationOptions {
            max_iterations: 100_000,
            tolerance: 1e-12,
        },
    };
    let r = designs::synthetic_control(&y, &donors, opts, &control()).unwrap();
    for (&a, b) in r.donor_weights.iter().zip(vector(&d["weights"])) {
        close(a, b, 2e-5);
    }
    close(r.post_effect, d["post_effect"].as_f64().unwrap(), 2e-5);
    close(r.donor_weights.iter().sum(), 1.0, 1e-12);
    for v in &mut y[20..] {
        *v += 100.0;
    }
    for v in &mut donors[0][20..] {
        *v -= 90.0;
    }
    let changed = designs::synthetic_control(&y, &donors, opts, &control()).unwrap();
    assert_eq!(r.donor_weights, changed.donor_weights);
}

#[test]
fn causal_estimators_reject_unidentified_inputs_and_honor_control() {
    let f = fixture();
    let d = &f["treatment"];
    let y = vector(&d["response"]);
    let t = vector(&d["treatment"]);
    let x = matrix(&d["predictors"]);
    assert!(
        treatment::estimate(
            &y,
            &vec![0.; y.len()],
            &x,
            options(TreatmentMethod::Ipw),
            &control()
        )
        .is_err()
    );
    assert!(
        treatment::estimate(
            &y,
            &t,
            &x,
            TreatmentOptions {
                overlap: 0.49,
                ..options(TreatmentMethod::Aipw)
            },
            &control()
        )
        .is_err()
    );
    assert!(
        treatment::estimate(
            &y,
            &t,
            &x,
            TreatmentOptions {
                caliper: 1e-15,
                ..options(TreatmentMethod::Matching)
            },
            &control()
        )
        .is_err()
    );
    assert!(
        treatment::estimate(
            &y,
            &t,
            &x,
            TreatmentOptions {
                bootstrap: BootstrapOptions {
                    replications: 1,
                    seed: 0
                },
                ..options(TreatmentMethod::RegressionAdjustment)
            },
            &control()
        )
        .is_err()
    );
    assert!(
        designs::rdd(
            &y,
            &x[0],
            RddOptions {
                cutoff: 0.,
                bandwidth: 0.,
                triangular: true
            },
            &control()
        )
        .is_err()
    );
    assert!(designs::heterogeneity(&y, &t, &vec![0; y.len()], &x, &control()).is_err());
    assert!(
        econometrics::gmm(
            &y,
            &x,
            &x[..1],
            GmmOptions {
                constant: true,
                two_step: true
            },
            &control()
        )
        .is_err()
    );
    assert!(
        econometrics::sur(
            &[y.clone(), y.clone()],
            &x,
            &[vec![0], vec![0, 0]],
            true,
            &control()
        )
        .is_err()
    );
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert_eq!(
        treatment::estimate(&y, &t, &x, options(TreatmentMethod::Aipw), &cancelled).unwrap_err(),
        ScientificComputationError::Cancelled
    );
    let expired = ScientificExecutionControl {
        deadline: Instant::now() - Duration::from_secs(1),
        ..control()
    };
    assert_eq!(
        designs::synthetic_control(
            &y,
            &x,
            SyntheticControlOptions {
                pre_periods: 10,
                iteration: IterationOptions::default()
            },
            &expired
        )
        .unwrap_err(),
        ScientificComputationError::DeadlineExceeded
    );
}
