use std::time::{Duration, Instant};
use yss_sci::decision::{fuzzy::evaluate, hierarchy::*, influence::*};
use yss_sci_contract::{
    decision::{fuzzy::FuzzyOperator, influence::*},
    execution::*,
};
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/decision_matrices_reference.json")).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1. + b.abs()), "{a} != {b}");
}
#[test]
fn judgment_weights_match_independent_eigen_references_without_a_matrix_order_cap() {
    let f = fixture();
    let a: Vec<Vec<f64>> = serde_json::from_value(f["ahp"]["columns"].clone()).unwrap();
    let fit = ahp(&a, 0., &control()).unwrap();
    for (i, w) in fit.weights.iter().enumerate() {
        close(*w, f["ahp"]["weights"][i].as_f64().unwrap());
    }
    close(
        fit.principal_eigenvalue,
        f["ahp"]["lambda"].as_f64().unwrap(),
    );
    close(
        fit.consistency_ratio.unwrap(),
        f["ahp"]["cr"].as_f64().unwrap(),
    );
    let a: Vec<Vec<f64>> = serde_json::from_value(f["fahp"]["columns"].clone()).unwrap();
    let fit = fuzzy_ahp(&a, &control()).unwrap();
    for (i, w) in fit.weights.iter().enumerate() {
        close(*w, f["fahp"]["weights"][i].as_f64().unwrap());
    }
    close(
        fit.compatibility_index,
        f["fahp"]["compatibility"].as_f64().unwrap(),
    );
    let large = vec![vec![1.; 16]; 16];
    let fit = ahp(&large, 0., &control()).unwrap();
    assert!(fit.random_index.is_none() && fit.consistency_ratio.is_none());
    assert!(fit.weights.iter().all(|w| (*w - 1. / 16.).abs() < 1e-12));
    let fit = ahp(&large, 1.6, &control()).unwrap();
    assert_eq!(fit.random_index, Some(1.6));
    close(fit.consistency_ratio.unwrap(), 0.);
    assert!(ahp(&[vec![1., 2.], vec![2., 1.]], 0., &control()).is_err());
    assert!(fuzzy_ahp(&[vec![0.5, 0.2], vec![0.2, 0.5]], &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        ahp(&large, 0., &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
#[test]
fn dematel_matches_numpy_and_rejects_nonconvergent_influence_series() {
    let f = fixture();
    let a: Vec<Vec<f64>> = serde_json::from_value(f["dematel"]["columns"].clone()).unwrap();
    let fit = dematel(&a, InfluenceNormalization::MaxSum, 1., &control()).unwrap();
    for row in &fit.rows {
        close(
            row.outgoing,
            f["dematel"]["outgoing"][row.criterion - 1]
                .as_f64()
                .unwrap(),
        );
        close(
            row.incoming,
            f["dematel"]["incoming"][row.criterion - 1]
                .as_f64()
                .unwrap(),
        );
    }
    for cell in &fit.matrix {
        close(
            cell.total,
            f["dematel"]["total"][cell.source - 1][cell.target - 1]
                .as_f64()
                .unwrap(),
        );
    }
    let cycle = [vec![0., 1.], vec![1., 0.]];
    assert!(dematel(&cycle, InfluenceNormalization::MaxSum, 1., &control()).is_err());
    let damped = dematel(&cycle, InfluenceNormalization::MaxSum, 0.8, &control()).unwrap();
    close(damped.matrix[0].total, 16. / 9.);
    close(damped.matrix[1].total, 20. / 9.);
    assert!(
        dematel(
            &[vec![0., 2.], vec![2., 0.]],
            InfluenceNormalization::None,
            1.,
            &control()
        )
        .is_err()
    );
    let zero = dematel(
        &vec![vec![0.; 3]; 3],
        InfluenceNormalization::MaxSum,
        1.,
        &control(),
    )
    .unwrap();
    assert!(
        zero.rows
            .iter()
            .all(|r| r.weight.is_none() && r.prominence == 0.)
    );
}
#[test]
fn ism_closure_and_levels_preserve_cycles_disconnected_factors_and_direction() {
    let chain = ism(
        &[vec![0., 0., 0.], vec![1., 0., 0.], vec![0., 1., 0.]],
        &control(),
    )
    .unwrap();
    assert_eq!(chain.summary.levels, [vec![3], vec![2], vec![1]]);
    assert_eq!(
        chain
            .rows
            .iter()
            .map(|r| r.driving_power)
            .collect::<Vec<_>>(),
        [3, 2, 1]
    );
    assert_eq!(
        chain.rows.iter().map(|r| r.dependence).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert!(chain.matrix[2].reachable);
    assert!(!chain.matrix[6].reachable);
    let cycle = ism(
        &[vec![0., 1., 0.], vec![1., 0., 0.], vec![0., 1., 0.]],
        &control(),
    )
    .unwrap();
    assert_eq!(cycle.summary.levels, [vec![3], vec![1, 2]]);
    assert_eq!(
        cycle.summary.strongly_connected_components,
        [vec![1, 2], vec![3]]
    );
    let disconnected = ism(&vec![vec![0.; 65]; 65], &control()).unwrap();
    assert_eq!(disconnected.summary.levels[0].len(), 65);
    assert!(
        disconnected
            .rows
            .iter()
            .all(|r| r.driving_power == 1 && r.dependence == 1)
    );
    assert!(ism(&[vec![0.5]], &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        ism(&[vec![0.]], &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
#[test]
fn fuzzy_operators_normalize_count_rows_and_preserve_tied_grades() {
    let columns = [vec![2., 6.], vec![8., 4.]];
    for (operator, expected, raw) in [
        (FuzzyOperator::ProductSum, [0.5, 0.5], [0.5, 0.5]),
        (FuzzyOperator::MinMax, [0.6, 0.4], [0.6, 0.4]),
        (FuzzyOperator::ProductMax, [0.6, 0.4], [0.45, 0.3]),
        (FuzzyOperator::MinSum, [16. / 29., 13. / 29.], [0.8, 0.65]),
    ] {
        let fit = evaluate(&columns, &[1., 3.], &[1., 5.], operator, &control()).unwrap();
        for j in 0..2 {
            close(fit.memberships[j], expected[j]);
            close(fit.raw_memberships[j], raw[j]);
        }
        close(fit.score.unwrap(), expected[0] + 5. * expected[1]);
    }
    let fit = evaluate(
        &columns,
        &[1., 3.],
        &[],
        FuzzyOperator::ProductSum,
        &control(),
    )
    .unwrap();
    assert_eq!(fit.dominant_grades, [1, 2]);
    assert!(fit.score.is_none());
    let fit = evaluate(
        &vec![vec![1.; 4]; 2],
        &[],
        &[],
        FuzzyOperator::MinSum,
        &control(),
    )
    .unwrap();
    assert_eq!(fit.raw_memberships, [1., 1.]);
    assert!(
        evaluate(
            &[vec![0.], vec![0.]],
            &[],
            &[],
            FuzzyOperator::ProductSum,
            &control()
        )
        .is_err()
    );
}
