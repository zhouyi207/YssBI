use std::time::{Duration, Instant};
use yss_sci::decision::experts::delphi;
use yss_sci_contract::execution::*;
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
#[test]
fn delphi_reuses_concordance_with_experts_on_rows_and_sample_quartiles() {
    let fit = delphi(
        &[
            vec![1., 2., 3., 1.],
            vec![2., 3., 4., 2.],
            vec![3., 4., 5., 5.],
        ],
        5.,
        &control(),
    )
    .unwrap();
    let w = fit.summary.concordance.as_ref().unwrap();
    assert_eq!((w.observations, w.raters), (3, 4));
    assert_eq!(w.coefficient, 1.);
    assert!((w.inference.p_value.unwrap() - (-4_f64).exp()).abs() < 1e-12);
    let a = &fit.rows[0];
    assert_eq!(a.mean, 1.75);
    assert!((a.standard_deviation.unwrap() - (2.75_f64 / 3.).sqrt()).abs() < 1e-12);
    assert_eq!((a.q1, a.median, a.q3), (1., 1.5, 2.25));
    assert_eq!(fit.rows[2].full_score_percent, 50.);
    let smallest = f64::from_bits(1);
    let tiny = delphi(&[vec![smallest; 2]], 5., &control()).unwrap();
    assert_eq!(
        tiny.rows[0].median.to_bits(),
        smallest.to_bits(),
        "equal finite observations must retain their sample median"
    );
    let one = delphi(&[vec![3.], vec![5.]], 5., &control()).unwrap();
    assert!(one.summary.concordance.is_none() && one.rows[0].standard_deviation.is_none());
    let zero = delphi(&[vec![-1., 1.]], 5., &control()).unwrap();
    assert!(zero.rows[0].coefficient_of_variation.is_none());
    let same = delphi(&[vec![1., 2.], vec![1., 2.]], 5., &control()).unwrap();
    assert_eq!(
        same.summary.concordance_undefined_reason,
        Some("no_within_expert_variation")
    );
    assert!(delphi(&[vec![6.]], 5., &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        delphi(&[vec![5.; 640]], 5., &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
