use std::time::{Duration, Instant};
use yss_sci::decision::preferences::*;
use yss_sci_contract::execution::*;
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
#[test]
fn nps_and_kano_follow_published_boundaries_and_keep_undefined_coefficients() {
    let score = nps(&[0., 6., 7., 8., 9., 10., 10., 10.], &control()).unwrap();
    assert_eq!(
        (score.detractors, score.passives, score.promoters),
        (2, 2, 4)
    );
    assert_eq!(score.net_promoter_score, 25.);
    assert_eq!(score.rating_counts[10], 3);
    assert_eq!(nps(&[0.], &control()).unwrap().net_promoter_score, -100.);
    for bad in [vec![], vec![10.1], vec![-1.], vec![f64::NAN]] {
        assert!(nps(&bad, &control()).is_err());
    }
    let f = (1..=5).flat_map(|i| [i as f64; 5]).collect::<Vec<_>>();
    let d = (1..=5).cycle().take(25).map(f64::from).collect::<Vec<_>>();
    let all = kano(&f, &d, &control()).unwrap();
    // All 25 cells in the classic table: A=3, O=1, M=3, I=9, R=7, Q=2.
    assert_eq!(
        all.categories.iter().map(|v| v.count).collect::<Vec<_>>(),
        [3, 1, 3, 9, 7, 2]
    );
    assert_eq!(all.better, Some(0.25));
    assert_eq!(all.worse, Some(-0.25));
    assert_eq!(all.dominant_categories, ["indifferent"]);
    let undefined = kano(&[1., 5.], &[1., 1.], &control()).unwrap();
    assert_eq!(undefined.coefficient_observations, 0);
    assert!(undefined.better.is_none() && undefined.worse.is_none());
    assert_eq!(undefined.dominant_categories, ["reverse", "questionable"]);
    assert!(kano(&[1.], &[1., 1.], &control()).is_err());
    assert!(kano(&[0.], &[1.], &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        nps(&[9.; 640], &c),
        Err(ScientificComputationError::Cancelled)
    ));
    assert!(matches!(
        kano(&[1.], &[2.], &c),
        Err(ScientificComputationError::Cancelled)
    ));
}

#[test]
fn rfm_uses_directional_midrank_quintiles_without_splitting_ties() {
    let x = (0..640).map(f64::from).collect::<Vec<_>>();
    let result = rfm(&[x.clone(), x.clone(), x.clone()], &control()).unwrap();
    assert_eq!(result.summary.recency_counts, [128; 5]);
    assert_eq!(result.summary.frequency_counts, [128; 5]);
    for (i, row) in result.rows.iter().enumerate() {
        assert_eq!(row.recency_score, 5 - i / 128);
        assert_eq!(row.frequency_score, i / 128 + 1);
        assert_eq!(row.monetary_score, row.frequency_score);
    }
    let tied = rfm(
        &[
            vec![1., 1., 2., 3., 3.],
            vec![2.; 5],
            vec![-10., -10., 0., 10., 10.],
        ],
        &control(),
    )
    .unwrap();
    assert_eq!(
        tied.rows
            .iter()
            .map(|r| r.recency_score)
            .collect::<Vec<_>>(),
        [5, 5, 3, 2, 2]
    );
    assert!(tied.rows.iter().all(|r| r.frequency_score == 3));
    let single = rfm(&[vec![0.], vec![0.], vec![0.]], &control()).unwrap();
    assert_eq!(single.rows[0].total, 9);
    assert!(rfm(&[vec![-1.], vec![0.], vec![0.]], &control()).is_err());
    assert!(rfm(&[vec![1.], vec![0.5], vec![0.]], &control()).is_err());
    let c = control();
    c.cancellation.cancel();
    assert!(matches!(
        rfm(&[x.clone(), x.clone(), x], &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
