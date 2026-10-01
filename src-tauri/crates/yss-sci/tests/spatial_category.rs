use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci::spatial::{moran, regression, weights};
use yss_sci_contract::{execution::*, regression::models::IterationOptions, spatial::*};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/spatial_category_reference.json")).unwrap()
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
fn compare(a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len());
    for (&a, &b) in a.iter().zip(b) {
        close(a, b, tol);
    }
}
fn compare_model(model: &SpatialRegressionResult, expected: &Value) {
    compare(
        &model
            .coefficients
            .iter()
            .map(|c| c.estimate)
            .collect::<Vec<_>>(),
        &vector(&expected["coefficients"]),
        2e-5,
    );
    for (a, b) in model.covariance.iter().zip(matrix(&expected["covariance"])) {
        compare(a, &b, 2e-5);
    }
    close(
        model.log_likelihood,
        expected["log_likelihood"].as_f64().unwrap(),
        2e-8,
    );
    close(
        model.sigma_squared,
        expected["sigma_squared"].as_f64().unwrap(),
        2e-6,
    );
    for (a, b) in model.impacts.iter().zip(matrix(&expected["impacts"])) {
        compare(&[a.direct, a.indirect, a.total], &b, 3e-5);
    }
}

#[test]
fn spatial_weights_rules_normalization_ties_and_islands() {
    let f = fixture();
    let d = &f["data"];
    let w = weights::construct(
        &vector(&d["x"]),
        &vector(&d["y_coordinate"]),
        WeightsOptions::default(),
        &control(),
    )
    .unwrap();
    assert_eq!(w.matrix, matrix(&d["weights"]));
    let x = [0., 1., 2., 20.];
    let y = [0.; 4];
    let options = WeightsOptions {
        rule: WeightRule::DistanceBand,
        radius: 1.1,
        ..Default::default()
    };
    let w = weights::construct(&x, &y, options, &control()).unwrap();
    assert_eq!(w.islands, vec![3]);
    assert_eq!(w.matrix[1], vec![0.5, 0., 0.5, 0.]);
    let w = weights::construct(
        &x,
        &y,
        WeightsOptions {
            rule: WeightRule::InverseDistance,
            radius: 3.,
            power: 2.,
            row_standardize: false,
            ..options
        },
        &control(),
    )
    .unwrap();
    assert_eq!(w.matrix[0][2], 0.25);
    let w = weights::construct(
        &x,
        &y,
        WeightsOptions {
            rule: WeightRule::Knn,
            neighbors: 1,
            symmetrize: true,
            ..options
        },
        &control(),
    )
    .unwrap();
    assert_eq!(w.matrix[1], vec![0.5, 0., 0.5, 0.]);
    assert_eq!(w.matrix[3][2], 1.);
    assert!(
        weights::construct(
            &[0., 0.],
            &[0., 0.],
            WeightsOptions {
                rule: WeightRule::InverseDistance,
                ..options
            },
            &control()
        )
        .is_err()
    );
}

#[test]
fn spatial_moran_matches_independent_moments_and_seeded_two_sided_permutations() {
    let f = fixture();
    let d = &f["data"];
    let y = vector(&d["response"]);
    let w = matrix(&d["weights"]);
    let m = moran::analyze(&y, &w, MoranOptions::default(), &control()).unwrap();
    let actual = serde_json::to_value(&m).unwrap();
    for key in [
        "statistic",
        "expected",
        "normal_variance",
        "randomization_variance",
        "normal_p_value",
        "randomization_p_value",
    ] {
        close(
            actual[key].as_f64().unwrap(),
            f["moran"][key].as_f64().unwrap(),
            1e-12,
        );
    }
    assert_eq!(
        m,
        moran::analyze(&y, &w, MoranOptions::default(), &control()).unwrap()
    );
    assert!(m.permutation_p_value.unwrap() >= 0.001);
    let full = (0..5)
        .map(|i| (0..5).map(|j| if i == j { 0. } else { 0.25 }).collect())
        .collect::<Vec<_>>();
    let degenerate = moran::analyze(
        &[1., 4., 2., 5., 3.],
        &full,
        MoranOptions {
            permutations: 99,
            seed: 8,
        },
        &control(),
    )
    .unwrap();
    close(degenerate.statistic, -0.25, 1e-14);
    assert_eq!(degenerate.normal_p_value, None);
    assert_eq!(degenerate.permutation_p_value, Some(1.));
}

#[test]
fn spatial_all_seven_models_match_scipy_estimates_information_predictions_and_impacts() {
    let f = fixture();
    let d = &f["data"];
    for (name, kind) in [
        ("ols", SpatialMethod::Ols),
        ("slx", SpatialMethod::Slx),
        ("slm", SpatialMethod::Slm),
        ("sem", SpatialMethod::Sem),
        ("sac", SpatialMethod::Sac),
        ("sdm", SpatialMethod::Sdm),
        ("sdem", SpatialMethod::Sdem),
    ] {
        let model = regression::fit(
            kind,
            &vector(&d["response"]),
            &matrix(&d["predictors"]),
            &matrix(&d["weights"]),
            SpatialOptions::default(),
            &control(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let expected = &f["models"][name];
        compare_model(&model, expected);
        compare(&model.fitted, &vector(&expected["fitted"]), 2e-5);
        compare(&model.innovations, &vector(&expected["innovations"]), 2e-5);
        compare(
            &model.reduced_fitted,
            &vector(&expected["reduced_fitted"]),
            2e-5,
        );
        for (j, c) in model.coefficients.iter().enumerate() {
            close(
                c.standard_error.unwrap().powi(2),
                model.covariance[j][j],
                1e-12,
            );
            assert!(c.p_value.unwrap() >= 0.);
        }
    }
}

#[test]
fn spatial_panel_orthogonal_likelihood_matches_reference_and_recovers_entity_effects() {
    let f = fixture();
    let d = &f["panel"];
    let w = matrix(&f["data"]["weights"]);
    let y = vector(&d["response"]);
    let x = matrix(&d["predictors"]);
    for (name, kind) in [("slm", SpatialMethod::Slm), ("sem", SpatialMethod::Sem)] {
        let m =
            regression::panel(kind, &y, &x, &w, IterationOptions::default(), &control()).unwrap();
        compare_model(&m, &d["models"][name]);
        assert_eq!(m.estimation_observations, w.len() * 4);
        for i in 0..w.len() {
            close(
                (0..5).map(|t| m.residuals[t * w.len() + i]).sum(),
                0.,
                1e-10,
            );
        }
        let shifted = y
            .iter()
            .enumerate()
            .map(|(i, y)| y + (i % w.len()) as f64 * 0.25)
            .collect::<Vec<_>>();
        let shifted = regression::panel(
            kind,
            &shifted,
            &x,
            &w,
            IterationOptions::default(),
            &control(),
        )
        .unwrap();
        compare(
            &m.coefficients
                .iter()
                .map(|c| c.estimate)
                .collect::<Vec<_>>(),
            &shifted
                .coefficients
                .iter()
                .map(|c| c.estimate)
                .collect::<Vec<_>>(),
            2e-6,
        );
    }
}

#[test]
fn spatial_rejects_bad_matrices_rank_missing_values_and_honors_execution_control() {
    let f = fixture();
    let d = &f["data"];
    let y = vector(&d["response"]);
    let x = matrix(&d["predictors"]);
    let mut w = matrix(&d["weights"]);
    w[0][0] = 1.;
    assert!(
        regression::fit(
            SpatialMethod::Slm,
            &y,
            &x,
            &w,
            SpatialOptions::default(),
            &control()
        )
        .is_err()
    );
    w[0][0] = 0.;
    w[0][1] = -1.;
    assert!(moran::analyze(&y, &w, MoranOptions::default(), &control()).is_err());
    let w = matrix(&d["weights"]);
    assert!(
        regression::fit(
            SpatialMethod::Sem,
            &y,
            &[vec![1.; y.len()]],
            &w,
            SpatialOptions::default(),
            &control()
        )
        .is_err()
    );
    assert!(moran::analyze(&vec![1.; y.len()], &w, MoranOptions::default(), &control()).is_err());
    assert!(
        regression::panel(
            SpatialMethod::Slm,
            &y[..y.len() - 1],
            &[],
            &w,
            IterationOptions::default(),
            &control()
        )
        .is_err()
    );
    let mut bad = y.clone();
    bad[0] = f64::NAN;
    assert!(moran::analyze(&bad, &w, MoranOptions::default(), &control()).is_err());
    let expired = ScientificExecutionControl {
        deadline: Instant::now() - Duration::from_secs(1),
        ..control()
    };
    assert!(matches!(
        moran::analyze(&y, &w, MoranOptions::default(), &expired),
        Err(ScientificComputationError::DeadlineExceeded)
    ));
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert!(matches!(
        regression::fit(
            SpatialMethod::Sac,
            &y,
            &x,
            &w,
            SpatialOptions::default(),
            &cancelled
        ),
        Err(ScientificComputationError::Cancelled)
    ));
}
