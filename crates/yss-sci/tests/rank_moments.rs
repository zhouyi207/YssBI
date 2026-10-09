use std::time::{Duration, Instant};
use yss_sci::hypothesis::nonparametric;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::{Alternative, RankHypothesisTest},
};

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}

#[test]
fn mann_kendall_uses_the_score_variance_for_tied_groups() {
    // Three observations with one pair tied: S=2 and Var(S)=(66-18)/18=8/3.
    for (values, sign) in [
        (vec![1.0, 1.0, 2.0], 1.0),
        (vec![2.0, 1.0, 1.0], -1.0),
        (vec![1.0, 1.0, 1.0], 0.0),
    ] {
        let report = nonparametric::run(
            RankHypothesisTest::MannKendall {
                values,
                alternative: Alternative::TwoSided,
            },
            &control(),
        )
        .unwrap();
        let expected = sign * (3.0_f64 / 8.0).sqrt();
        assert!((report.statistic.unwrap() - expected).abs() < 1e-12);
        assert_eq!(report.details["s_statistic"], sign * 2.0);
        assert!((report.details["kendall_tau"] - sign * 2.0 / 3.0).abs() < 1e-12);
    }
}

#[test]
fn mann_kendall_compression_treats_signed_zero_as_one_tied_value() {
    for values in [vec![-0.0, 0.0, 1.0], vec![0.0, -0.0, 1.0]] {
        let report = nonparametric::run(
            RankHypothesisTest::MannKendall {
                values,
                alternative: Alternative::TwoSided,
            },
            &control(),
        )
        .unwrap();
        assert!((report.statistic.unwrap() - (3.0_f64 / 8.0).sqrt()).abs() < 1e-12);
        assert_eq!(report.details["s_statistic"], 2.0);
    }
}

#[test]
fn large_rank_samples_compute_moments_without_integer_overflow() {
    let n = 1 << 21;
    let trend = nonparametric::run(
        RankHypothesisTest::MannKendall {
            values: (0..n).map(|i| i as f64).collect(),
            alternative: Alternative::TwoSided,
        },
        &control(),
    )
    .unwrap();
    assert!(trend.statistic.is_some_and(|z| z.is_finite() && z > 0.0));
    // Independent 70-digit Decimal reference for the two closed-form sample patterns.
    assert!((trend.statistic.unwrap() - 2172.23021915383).abs() < 1e-9);
    assert_eq!(trend.details["kendall_tau"], 1.0);
    assert_eq!(trend.p_value, 0.0);

    let runs = nonparametric::run(
        RankHypothesisTest::Runs {
            values: (0..3_000_000).map(|i| (i % 2) as f64).collect(),
        },
        &control(),
    )
    .unwrap();
    assert!(runs.statistic.is_some_and(|z| z.is_finite() && z > 0.0));
    assert!((runs.statistic.unwrap() - 1732.0499415434495).abs() < 1e-9);
    assert_eq!(runs.p_value, 0.0);
}
