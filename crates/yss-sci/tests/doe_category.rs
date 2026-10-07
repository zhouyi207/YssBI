use std::time::{Duration, Instant};
use yss_sci::doe::range_analysis;
use yss_sci_contract::execution::*;
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10 * (1. + b.abs()), "{a} != {b}")
}
#[test]
fn range_analysis_recovers_orthogonal_main_effects_and_marks_confounded_or_tied_levels() {
    let response: Vec<_> = (0..640).map(|i| [1., 3., 5., 7.][i % 4]).collect();
    let factors: Vec<Vec<usize>> = vec![
        (0..640).map(|i| (i % 4) / 2).collect(),
        (0..640).map(|i| i % 2).collect(),
        (0..640).map(|i| [(0), (1), (1), (0)][i % 4]).collect(),
    ];
    let c = control();
    let fit = range_analysis(&response, &factors, true, &c).unwrap();
    assert_eq!(fit.summary.observations, 640);
    assert_eq!(fit.summary.pairwise_orthogonal, Some(true));
    assert!(fit.summary.equal_level_counts);
    for (j, &expected) in [4., 2., 0.].iter().enumerate() {
        close(fit.summary.factors[j].range, expected);
        assert!(fit.summary.factors[j].balanced);
    }
    assert_eq!(fit.summary.factors[0].optimal_levels, [2]);
    assert_eq!(fit.summary.factors[2].optimal_levels, [1, 2]);
    close(fit.rows[0].mean, 2.);
    close(fit.rows[0].total, 640.);
    assert_eq!(fit.rows[0].observations, 320);
    let fit = range_analysis(&[10., 10., 14.], &[vec![0, 1, 1]], false, &c).unwrap();
    assert_eq!(fit.summary.pairwise_orthogonal, None);
    assert!(!fit.summary.factors[0].balanced);
    assert_eq!(fit.summary.factors[0].optimal_levels, [1]);
    close(fit.rows[1].mean, 12.);
    let fit = range_analysis(
        &response,
        &[factors[0].clone(), factors[0].clone()],
        true,
        &c,
    )
    .unwrap();
    assert_eq!(fit.summary.pairwise_orthogonal, Some(false));
    let fit = range_analysis(&[3.; 640], &factors, true, &c).unwrap();
    assert!(
        fit.summary
            .factors
            .iter()
            .all(|f| f.range == 0. && f.optimal_levels == [1, 2])
    );
    assert!(range_analysis(&[1., 2.], &[vec![0, 2]], true, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        range_analysis(&response, &factors, true, &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
