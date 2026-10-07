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
