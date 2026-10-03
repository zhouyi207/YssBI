use std::time::{Duration, Instant};
use yss_sci::decision::{ranking, weights};
use yss_sci_contract::{decision::*, execution::*};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/decision_reference.json")).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1. + b.abs()), "{a} != {b}");
}
fn weight(s: &str) -> WeightMethod {
    match s {
        "entropy" => WeightMethod::Entropy,
        "critic" => WeightMethod::Critic,
        "information" => WeightMethod::Information,
        "independence" => WeightMethod::Independence,
        "explicit" => WeightMethod::Explicit,
        _ => WeightMethod::Equal,
    }
}

#[test]
fn vikor_matches_pymcdm_and_reports_advantage_stability_and_tied_compromises() {
    use yss_sci::decision::compromise::vikor;
    let f = fixture();
    let x: Vec<Vec<f64>> = serde_json::from_value(f["columns"].clone()).unwrap();
    for case in f["vikor"].as_array().unwrap() {
        let fit = vikor(
            &x,
            &[false, true, false],
            &[0.5, 0.3, 0.2],
            case["v"].as_f64().unwrap(),
            &control(),
        )
        .unwrap();
        for (r, expected) in fit.rows.iter().zip(case["scores"].as_array().unwrap()) {
            close(r.compromise_score, expected.as_f64().unwrap());
        }
        assert!(!fit.summary.compromise_alternatives.is_empty());
    }
    let simple = vikor(&[vec![0., 0.5, 1.]], &[false], &[], 0.5, &control()).unwrap();
    assert!(simple.summary.acceptable_advantage && simple.summary.acceptable_stability);
    assert_eq!(simple.summary.compromise_alternatives, [3]);
    let flat = vikor(&[vec![4.; 3]], &[false], &[], 0.5, &control()).unwrap();
    assert_eq!(flat.summary.compromise_alternatives, [1, 2, 3]);
    assert!(
        flat.rows
            .iter()
            .all(|r| r.compromise_score == 0. && r.rank == 2.)
    );
    assert!(vikor(&x, &[false, true, false], &[], 1.1, &control()).is_err());
}

#[test]
fn system_evaluations_keep_undefined_coupling_and_obstacle_shares_as_null() {
    use yss_sci::decision::systems::*;
    let fit = coupling(
        &[vec![0.25, 0.25, 0., 0.], vec![0.25, 1., 1., 0.]],
        &[],
        &control(),
    )
    .unwrap();
    close(fit.rows[0].coupling.unwrap(), 1.);
    close(fit.rows[0].coordination_degree, 0.5);
    close(fit.rows[1].coupling.unwrap(), 0.8);
    close(fit.rows[1].coordination_degree, 0.5_f64.sqrt());
    close(fit.rows[2].coupling.unwrap(), 0.);
    assert!(fit.rows[3].coupling.is_none());
    close(fit.rows[3].coordination_degree, 0.);
    assert_eq!(fit.summary.undefined_rows, 1);
    assert!(coupling(&[vec![1.1], vec![0.5]], &[], &control()).is_err());
    let fit = obstacles(
        &[vec![0.2, 0.8, 1.], vec![0.4, 0.8, 1.]],
        &[false, false],
        &[0.25, 0.75],
        false,
        &control(),
    )
    .unwrap();
    close(fit.rows[0].obstacle_percent.unwrap(), 100. * 0.2 / 0.65);
    close(fit.rows[1].obstacle_percent.unwrap(), 100. * 0.45 / 0.65);
    close(fit.rows[2].obstacle_percent.unwrap(), 25.);
    assert!(fit.rows[4].obstacle_percent.is_none());
    assert!(fit.rows[5].obstacle_percent.is_none());
    assert_eq!(fit.summary.undefined_rows, 1);
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        coupling(&[vec![0.5], vec![0.5]], &[], &c),
        Err(ScientificComputationError::Cancelled)
    ));
}

#[test]
fn decision_weights_and_rankings_match_independent_references() {
    let f = fixture();
    let columns: Vec<Vec<f64>> = serde_json::from_value(f["columns"].clone()).unwrap();
    let costs = vec![false, true, false];
    let explicit = vec![0.5, 0.3, 0.2];
    for (method, expected) in f["weights"].as_object().unwrap() {
        let result =
            weights::calculate(&columns, &costs, weight(method), &explicit, &control()).unwrap();
        for (w, e) in result.weights.iter().zip(expected.as_array().unwrap()) {
            close(*w, e.as_f64().unwrap());
        }
    }
    for case in f["cases"].as_array().unwrap() {
        let method = match case["method"].as_str().unwrap() {
            "topsis" | "entropy_topsis" => RankingMethod::Topsis,
            "grey_relational" => RankingMethod::GreyRelational,
            "wrsr" => RankingMethod::Wrsr,
            "efficacy_coefficient" => RankingMethod::Efficacy,
            _ => RankingMethod::Composite,
        };
        let result = ranking::calculate(
            &columns,
            RankingOptions {
                method,
                weighting: weight(case["weighting"].as_str().unwrap()),
                normalization: match case["normalization"].as_str().unwrap() {
                    "none" => Normalization::None,
                    "vector" => Normalization::Vector,
                    _ => Normalization::MinMax,
                },
                costs: costs.clone(),
                weights: explicit.clone(),
                grey_resolution: 0.5,
            },
            &control(),
        )
        .unwrap();
        for (j, row) in result.rows.iter().enumerate() {
            close(row.score, case["scores"][j].as_f64().unwrap());
            close(row.rank, case["ranks"][j].as_f64().unwrap());
        }
    }
}
#[test]
fn decision_handles_scaling_constant_criteria_invalid_weights_and_cancellation() {
    let single = ranking::calculate(
        &[vec![2.], vec![3.]],
        RankingOptions {
            method: RankingMethod::Composite,
            weighting: WeightMethod::Equal,
            normalization: Normalization::None,
            costs: vec![false, false],
            weights: vec![],
            grey_resolution: 0.5,
        },
        &control(),
    )
    .unwrap();
    close(single.rows[0].score, 2.5);
    close(single.rows[0].rank, 1.);
    let f = fixture();
    let columns: Vec<Vec<f64>> = serde_json::from_value(f["columns"].clone()).unwrap();
    let mut large = columns
        .iter()
        .map(|x| {
            x.iter()
                .cycle()
                .take(640)
                .map(|v| v * 1e250)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let options = RankingOptions {
        method: RankingMethod::Topsis,
        weighting: WeightMethod::Entropy,
        normalization: Normalization::Vector,
        costs: vec![false, true, false],
        weights: vec![],
        grey_resolution: 0.5,
    };
    let fit = ranking::calculate(&large, options.clone(), &control()).unwrap();
    assert_eq!(fit.rows.len(), 640);
    close(
        fit.rows[639].score,
        f["cases"][10]["scores"][7].as_f64().unwrap(),
    );
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        ranking::calculate(&large, options.clone(), &c),
        Err(ScientificComputationError::Cancelled)
    ));
    large[0].pop();
    assert!(ranking::calculate(&large, options, &control()).is_err());
    assert!(
        weights::calculate(
            &columns,
            &[false, true, false],
            WeightMethod::Explicit,
            &[0., 0., 0.],
            &control()
        )
        .is_err()
    );
    assert!(
        weights::calculate(
            &[vec![1.; 8], vec![1.; 8]],
            &[false, false],
            WeightMethod::Entropy,
            &[],
            &control()
        )
        .is_err()
    );
    let mut with_constant = columns.clone();
    with_constant.push(vec![1.; 8]);
    let result = weights::calculate(
        &with_constant,
        &[false, true, false, false],
        WeightMethod::Entropy,
        &[],
        &control(),
    )
    .unwrap();
    close(result.weights[3], 0.);
    assert!(
        weights::calculate(
            &[vec![1.; 8], columns[0].clone()],
            &[false, false],
            WeightMethod::Independence,
            &[],
            &control()
        )
        .is_err()
    );
}
