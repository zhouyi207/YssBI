use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci_contract::{
    diagnostics::model::*,
    execution::*,
    regression::{
        OlsOptions,
        discrete::BinaryOptions,
        fit::{BinaryRegressionLink, RegressionFit},
        linear::{LinearRegressionMethod, LinearRegressionRequest, LinearRegressionResult},
    },
    survival::*,
};
use yss_sci_runtime::diagnostics::{comparison::*, design::*, influence::*, reclassification::*};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../yss-sci/tests/fixtures/diagnostics_reference.json"
    ))
    .unwrap()
}
fn vector(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn matrix(v: &Value) -> Vec<Vec<f64>> {
    serde_json::from_value(v.clone()).unwrap()
}
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}
fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol * (1.0 + b.abs()), "{a} != {b}");
}
fn linear(f: &Value, k: usize, weighted: bool) -> LinearRegressionResult {
    yss_sci_runtime::regression::linear::linear_regression(
        LinearRegressionRequest {
            response: vector(&f["y"]),
            predictors: matrix(&f["x"])[..k].to_vec(),
            options: OlsOptions::default(),
            method: if weighted {
                LinearRegressionMethod::Wls {
                    weights: vector(&f["weights"]),
                }
            } else {
                LinearRegressionMethod::Ols
            },
        },
        &control(),
    )
    .unwrap()
}
fn binary(f: &Value, k: usize, link: BinaryRegressionLink) -> RegressionFit {
    let y = vector(&f["binary_y"]);
    let n = y.len();
    yss_sci::regression::discrete::fit::fit_binary(
        link,
        y,
        &matrix(&f["x"])[..k],
        BinaryOptions::default(),
        yss_sci_contract::StatisticalObservationMetadata {
            original_observation_count: n,
            used_observation_count: n,
            dropped_null_count: 0,
            dropped_nan_count: 0,
            missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
        },
    )
    .unwrap()
}
fn check_ic(actual: &InformationCriteria, expected: &Value) {
    assert_eq!(
        actual.parameters,
        expected["parameters"].as_u64().unwrap() as usize
    );
    for (a, key) in [
        (actual.log_likelihood, "log_likelihood"),
        (actual.aic, "aic"),
        (actual.bic, "bic"),
    ] {
        close(a, expected[key].as_f64().unwrap(), 1e-8);
    }
}

#[test]
fn gaussian_criteria_comparison_and_influence_match_statsmodels_above_512_rows() {
    let f = fixture();
    assert!(f["y"].as_array().unwrap().len() > 512);
    for (name, weighted) in [("ols", false), ("wls", true)] {
        let r = linear(&f, 1, weighted);
        let full = linear(&f, 3, weighted);
        let expected = &f["gaussian"][name];
        check_ic(
            &information_criteria(DiagnosticModel::Linear(&full), &control()).unwrap(),
            &expected["criteria"],
        );
        let compared = compare_models(
            DiagnosticModel::Linear(&r),
            DiagnosticModel::Linear(&full),
            ComparisonMethod::All,
            &control(),
        )
        .unwrap();
        for (result, key) in [
            (compared.likelihood_ratio.unwrap(), "lr"),
            (compared.score.unwrap(), "lm"),
            (compared.f_test.unwrap(), "f"),
        ] {
            close(result.statistic, expected[key][0].as_f64().unwrap(), 1e-7);
            close(result.p_value, expected[key][1].as_f64().unwrap(), 1e-8);
            assert_eq!(result.degrees_of_freedom, 2);
        }
        let influence = influence(&full, &control()).unwrap();
        let to_options = influence
            .leverage
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>();
        for (values, key) in [
            (&to_options, "leverage"),
            (&influence.standardized_residuals, "standardized"),
            (&influence.studentized_residuals, "studentized"),
            (&influence.cooks_distance, "cooks"),
        ] {
            for (a, b) in values.iter().zip(vector(&expected[key])) {
                close(a.unwrap(), b, 1e-7);
            }
        }
        close(influence.leverage.iter().sum(), 4.0, 1e-10);
    }
}

#[test]
fn binary_likelihood_and_expected_information_score_match_logit_and_probit_references() {
    let f = fixture();
    for (name, link) in [
        ("logit", BinaryRegressionLink::Logit),
        ("probit", BinaryRegressionLink::Probit),
    ] {
        let restricted = binary(&f, 1, link);
        let full = binary(&f, 3, link);
        let expected = &f["binary"][name];
        let r = compare_models(
            DiagnosticModel::Binary(&restricted),
            DiagnosticModel::Binary(&full),
            ComparisonMethod::All,
            &control(),
        )
        .unwrap();
        check_ic(&r.full, &expected["criteria"]);
        assert!(r.f_test.is_none());
        for (test, key, p_key) in [
            (r.likelihood_ratio.unwrap(), "lr", "lr_p"),
            (r.score.unwrap(), "lm", "lm_p"),
        ] {
            close(test.statistic, expected[key].as_f64().unwrap(), 1e-7);
            close(test.p_value, expected[p_key].as_f64().unwrap(), 1e-8);
        }
    }
}

#[test]
fn design_diagnostics_match_numpy_and_report_exact_dependence_without_fake_infinite_numbers() {
    let f = fixture();
    let mut x = matrix(&f["x"]);
    let c = collinearity(&x, true, &control()).unwrap();
    assert!(c.full_column_rank);
    close(
        c.condition_number.unwrap(),
        f["collinearity"]["condition"].as_f64().unwrap(),
        1e-9,
    );
    for (term, v) in c.terms[1..].iter().zip(vector(&f["collinearity"]["vif"])) {
        close(term.vif.unwrap(), v, 1e-9);
    }
    let h = harman(&x, &control()).unwrap();
    for (a, b) in h
        .eigenvalues
        .iter()
        .zip(vector(&f["harman"]["eigenvalues"]))
    {
        close(*a, b, 1e-9);
    }
    close(
        h.first_component_ratio,
        f["harman"]["ratio"][0].as_f64().unwrap(),
        1e-9,
    );
    x.push(x[0].clone());
    let c = collinearity(&x, true, &control()).unwrap();
    assert!(!c.full_column_rank);
    assert_eq!(c.rank, 4);
    assert!(c.condition_number.is_none());
    assert!(c.terms[1].vif.is_none());
    assert_eq!(c.terms[1].tolerance, Some(0.0));
    assert!(c.terms[2].vif.is_some());
    // A resolvable tiny singular value must produce a large finite VIF,
    // rather than being lost to roundoff in the Gram matrix eigenvalues.
    let a = vec![-1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0];
    let perturbation = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
    let b = a
        .iter()
        .zip(perturbation)
        .map(|(a, e)| a + 1e-7 * e)
        .collect();
    let near = collinearity(&[a, b], true, &control()).unwrap();
    assert!(near.full_column_rank);
    close(near.condition_number.unwrap(), 2e7, 1e-7);
    for term in &near.terms[1..] {
        close(term.vif.unwrap(), 1e14 + 1.0, 1e-7);
    }
    let wide = collinearity(
        &[
            vec![1., 2., 3.],
            vec![2., 1., 4.],
            vec![4., 2., 8.],
            vec![1., 3., 7.],
        ],
        true,
        &control(),
    )
    .unwrap();
    assert_eq!(wide.rank, 3);
    assert_eq!(wide.eigenvalues.len(), 5);
    assert!(!wide.full_column_rank);
    assert!(harman(&[vec![1.; 10], vec![2.; 10]], &control()).is_err());
}

#[test]
fn reclassification_counts_use_paired_binary_outcomes_and_explicit_boundary_rules() {
    let y = [1., 1., 1., 1., 0., 0., 0., 0.];
    let old = [0.1, 0.5, 0.8, 0.2, 0.7, 0.5, 0.2, 0.0];
    let new = [0.2, 0.4, 0.9, 0.2, 0.5, 0.6, 0.1, -0.0];
    let c = nri_idi(
        &y,
        &old,
        &new,
        ReclassificationMode::Continuous,
        &[],
        &control(),
    )
    .unwrap();
    assert_eq!(
        (c.events.upward, c.events.downward, c.events.unchanged),
        (2, 1, 1)
    );
    assert_eq!(
        (
            c.nonevents.upward,
            c.nonevents.downward,
            c.nonevents.unchanged
        ),
        (1, 2, 1)
    );
    close(c.nri, 0.5, 1e-12);
    close(c.idi, 0.075, 1e-12);
    let c = nri_idi(
        &y,
        &old,
        &new,
        ReclassificationMode::Categorical,
        &[0.2, 0.5],
        &control(),
    )
    .unwrap();
    assert_eq!((c.events.upward, c.events.downward), (1, 1));
    assert_eq!((c.nonevents.upward, c.nonevents.downward), (0, 1));
    close(c.nri, 0.25, 1e-12);
    for cuts in [vec![], vec![0.5, 0.2], vec![0.2, 0.2], vec![0.0]] {
        assert!(
            nri_idi(
                &y,
                &old,
                &new,
                ReclassificationMode::Categorical,
                &cuts,
                &control()
            )
            .is_err()
        );
    }
}

#[test]
fn proportional_hazards_score_matches_independent_counting_process_information() {
    let f = fixture();
    let d = &f["survival"];
    let time = vector(&d["time"]);
    let event = vector(&d["event"]);
    let x = matrix(&d["predictors"]);
    for expected in f["ph"].as_array().unwrap() {
        let ties = if expected["ties"] == "efron" {
            CoxTies::Efron
        } else {
            CoxTies::Breslow
        };
        let transform = match expected["transform"].as_str().unwrap() {
            "rank" => PhTimeTransform::Rank,
            "log" => PhTimeTransform::Log,
            _ => PhTimeTransform::Identity,
        };
        let r = yss_sci_runtime::survival::cox::proportional_hazards(
            &time,
            &event,
            &x,
            CoxOptions {
                ties,
                ..Default::default()
            },
            transform,
            &control(),
        )
        .unwrap();
        close(
            r.global.statistic,
            expected["global_statistic"].as_f64().unwrap(),
            2e-6,
        );
        close(
            r.global.p_value,
            expected["global_p"].as_f64().unwrap(),
            2e-6,
        );
        for (j, term) in r.terms.iter().enumerate() {
            close(
                term.test.statistic,
                expected["terms"][j].as_f64().unwrap(),
                2e-6,
            );
            close(
                term.test.p_value,
                expected["terms_p"][j].as_f64().unwrap(),
                2e-6,
            );
        }
    }
}

#[test]
fn model_comparisons_reject_mismatched_samples_nonnesting_and_invalid_execution() {
    let f = fixture();
    let full = linear(&f, 3, false);
    let restricted = linear(&f, 1, false);
    let compare = |r: &LinearRegressionResult, f: &LinearRegressionResult| {
        compare_models(
            DiagnosticModel::Linear(r),
            DiagnosticModel::Linear(f),
            ComparisonMethod::All,
            &control(),
        )
    };
    let mut different = restricted.clone();
    different.residuals[0] += 1.0;
    assert!(compare(&different, &full).is_err());
    different = restricted.clone();
    different.design[1][0] += 1.0;
    assert!(compare(&different, &full).is_err());
    assert!(compare(&full, &restricted).is_err());
    assert!(compare(&linear(&f, 1, true), &full).is_err());
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert_eq!(
        influence(&full, &cancelled).unwrap_err(),
        ScientificComputationError::Cancelled
    );
    assert_eq!(
        information_criteria(DiagnosticModel::Linear(&full), &cancelled).unwrap_err(),
        ScientificComputationError::Cancelled
    );
    let mut deadline = control();
    deadline.deadline = Instant::now();
    assert_eq!(
        harman(&matrix(&f["x"]), &deadline).unwrap_err(),
        ScientificComputationError::DeadlineExceeded
    );
}
