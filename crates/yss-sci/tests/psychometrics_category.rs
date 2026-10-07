use std::time::{Duration, Instant};
use yss_sci::psychometrics::*;
use yss_sci_contract::execution::*;
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}
fn data() -> Vec<Vec<f64>> {
    [
        (1., 0.07, 0.13, 17, 11),
        (2., 0.11, 0.17, 19, 13),
        (0.4, 0.09, 0.05, 23, 17),
        (3., 0.15, 0.11, 29, 19),
    ]
    .into_iter()
    .map(|(base, slope, noise, a, m)| {
        (0..640)
            .map(|i| base + slope * ((i * 13) % 37) as f64 + noise * ((i * a) % m) as f64)
            .collect()
    })
    .collect()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * (1. + b.abs()), "{a} != {b}")
}
#[test]
fn scale_statistics_and_tail_tests_match_numpy_scipy() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/psychometrics_reference.json")).unwrap();
    let number = |key: &str| f[key].as_f64().unwrap();
    let cell = |key: &str, j: usize| f[key][j].as_f64().unwrap();
    let x = data();
    let c = control();
    let r = reliability(&x, &c).unwrap();
    close(r.summary.raw_alpha.unwrap(), number("raw_alpha"));
    close(
        r.summary.standardized_alpha.unwrap(),
        number("standardized_alpha"),
    );
    for (j, row) in r.rows.iter().enumerate() {
        close(row.mean, cell("means", j));
        close(row.standard_deviation, cell("sd", j));
        close(
            row.corrected_item_total_correlation.unwrap(),
            cell("corrected_correlations", j),
        );
        close(row.alpha_if_deleted.unwrap(), cell("alpha_deleted", j));
    }
    let v = validity(&x, &c).unwrap();
    close(v.kmo.unwrap(), number("kmo"));
    close(v.bartlett_chi_square, number("bartlett"));
    assert_eq!(v.bartlett_df, 6);
    close(v.bartlett_p_value, number("bartlett_p"));
    for (j, msa) in v.item_msa.iter().enumerate() {
        close(msa.unwrap(), cell("msa", j));
    }
    let r = item_analysis(&x, 0.27, &c).unwrap();
    assert_eq!(r.scores.len(), 640);
    assert_eq!(
        r.summary.low_count,
        f["low_count"].as_u64().unwrap() as usize
    );
    assert_eq!(
        r.summary.high_count,
        f["high_count"].as_u64().unwrap() as usize
    );
    close(r.summary.low_cutoff, number("low_cutoff"));
    close(r.summary.high_cutoff, number("high_cutoff"));
    for (j, row) in r.rows.iter().enumerate() {
        close(row.low_mean.unwrap(), cell("low_mean", j));
        close(row.high_mean.unwrap(), cell("high_mean", j));
        close(row.t_statistic.unwrap(), cell("t", j));
        close(row.degrees_of_freedom.unwrap(), cell("df", j));
        close(row.p_value.unwrap(), cell("p", j));
    }
}
#[test]
fn constant_and_tied_items_keep_undefined_results_and_disjoint_groups() {
    let c = control();
    let x = vec![vec![1.; 640], vec![2.; 640]];
    let r = item_analysis(&x, 0.27, &c).unwrap();
    assert!(
        r.summary.reliability.raw_alpha.is_none()
            && r.summary.reliability.standardized_alpha.is_none()
    );
    assert_eq!((r.summary.low_count, r.summary.high_count), (0, 0));
    assert!(r.scores.iter().all(|r| r.group == 0));
    assert!(r.rows.iter().all(|r| r.t_statistic.is_none()
        && r.reliability.corrected_item_total_correlation.is_none()
        && r.reliability.alpha_if_deleted.is_none()));
    let r = item_analysis(
        &[vec![0., 1., 1., 1., 1., 1., 1., 2.], vec![0.; 8]],
        0.27,
        &c,
    )
    .unwrap();
    assert_eq!((r.summary.low_count, r.summary.high_count), (1, 1));
    assert_eq!(
        r.scores.iter().map(|r| r.group).collect::<Vec<_>>(),
        [-1, 0, 0, 0, 0, 0, 0, 1]
    );
    let r = item_analysis(
        &[vec![0., 0., 0., 1., 2., 2., 2., 2.], vec![0.; 8]],
        0.27,
        &c,
    )
    .unwrap();
    assert_eq!((r.summary.low_count, r.summary.high_count), (3, 4));
    assert!(r.rows[0].t_statistic.is_none());
    let r = reliability(&[vec![1., 2., 4., 6.], vec![6., 4., 2., 1.]], &c).unwrap();
    assert!(r.summary.raw_alpha.unwrap() < 0.);
    assert!(validity(&x, &c).is_err());
    assert!(validity(&[vec![1., 2., 3., 4.], vec![2., 4., 6., 8.]], &c).is_err());
    assert!(item_analysis(&data(), 0.5, &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        reliability(&data(), &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
#[test]
fn expert_relevance_uses_binomial_chance_and_distinct_scale_cvi_definitions() {
    let c = control();
    let r = content_validity(&[vec![3., 4., 3., 4., 3., 2., 2., 1.], vec![4.; 8]], &c).unwrap();
    close(r.rows[0].item_cvi, 0.625);
    close(r.rows[0].chance_agreement, 56. / 256.);
    close(r.rows[0].modified_kappa, 0.52);
    close(r.summary.scale_cvi_average, 0.8125);
    close(r.summary.scale_cvi_universal_agreement, 0.5);
    let r = content_validity(&[vec![3.; 640]], &c).unwrap();
    assert_eq!(r.summary.experts, 640);
    assert!(r.rows[0].chance_agreement > 0.);
    close(r.rows[0].modified_kappa, 1.);
    assert!(content_validity(&[vec![0., 5.]], &c).is_err());
    c.cancellation.cancel();
    assert!(matches!(
        content_validity(&[vec![3.; 640]], &c),
        Err(ScientificComputationError::Cancelled)
    ));
}
