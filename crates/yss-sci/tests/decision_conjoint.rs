use std::time::{Duration, Instant};
use yss_sci::decision::conjoint::fit;
use yss_sci_contract::execution::*;
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1. + b.abs()), "{a} != {b}");
}
#[test]
fn centered_utilities_covariance_and_predictions_match_statsmodels() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/conjoint_reference.json")).unwrap();
    let a: Vec<usize> = (0..648).map(|i| i % 2).collect();
    let b: Vec<usize> = (0..648).map(|i| (i / 2) % 3).collect();
    let y: Vec<f64> = (0..648)
        .map(|i| {
            3. + 1.4 * a[i] as f64
                + if b[i] == 1 {
                    0.8
                } else if b[i] == 2 {
                    -0.5
                } else {
                    0.
                }
                + (((i * 17) % 23) as f64 - 11.) * 0.07
        })
        .collect();
    let result = fit(&y, &[a, b], &control()).unwrap();
    assert_eq!(
        (
            result.summary.observations,
            result.summary.parameters,
            result.summary.residual_degrees_of_freedom
        ),
        (648, 4, 644)
    );
    close(
        result.summary.centered_intercept,
        f["intercept"].as_f64().unwrap(),
    );
    close(
        result.summary.r_squared.unwrap(),
        f["r_squared"].as_f64().unwrap(),
    );
    for (j, level) in result
        .summary
        .attributes
        .iter()
        .flat_map(|a| &a.levels)
        .enumerate()
    {
        close(level.utility, f["utilities"][j].as_f64().unwrap());
        close(
            level.standard_error.unwrap(),
            f["standard_errors"][j].as_f64().unwrap(),
        );
    }
    for (j, a) in result.summary.attributes.iter().enumerate() {
        close(
            a.importance_percent.unwrap(),
            f["importance"][j].as_f64().unwrap(),
        );
        close(a.levels.iter().map(|l| l.utility).sum(), 0.);
    }
    close(result.rows[647].fitted, f["last_fitted"].as_f64().unwrap());
    close(
        result.rows[647].residual,
        f["last_residual"].as_f64().unwrap(),
    );
}
#[test]
fn identifiable_saturated_and_constant_designs_do_not_fabricate_inference() {
    let c = control();
    let saturated = fit(&[2., 6.], &[vec![0, 1]], &c).unwrap();
    close(saturated.summary.centered_intercept, 4.);
    close(saturated.summary.attributes[0].levels[0].utility, -2.);
    assert!(
        saturated.summary.attributes[0]
            .levels
            .iter()
            .all(|l| l.standard_error.is_none())
    );
    let constant = fit(
        &[3.; 642],
        &[(0..642).map(|i| i % 2).collect(), vec![0; 642]],
        &c,
    )
    .unwrap();
    assert!(constant.summary.r_squared.is_none());
    assert!(constant.summary.attributes.iter().all(|a| {
        a.importance_percent.is_none()
            && a.levels
                .iter()
                .all(|l| l.utility == 0. && l.standard_error == Some(0.))
    }));
    assert!(fit(&[1., 2., 3., 4.], &[vec![0, 1, 0, 1], vec![0, 1, 0, 1]], &c).is_err());
    assert!(fit(&[1., 2., 3.], &[vec![0, 2, 2]], &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        fit(&[1., 2.], &[vec![0, 1]], &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
