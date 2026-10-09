use std::time::{Duration, Instant};
use yss_sci::hypothesis::nonparametric;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::RankHypothesisTest,
};

#[test]
fn mood_median_preserves_group_margins_and_ignores_pooled_median_ties() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    for (groups, median, degrees, p) in [
        (
            vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
            3.5,
            1.0,
            0.014_305_878_435_429_648,
        ),
        (
            vec![
                vec![1.0, 2.0, 3.0],
                vec![4.0, 5.0, 6.0],
                vec![7.0, 8.0, 9.0],
            ],
            5.0,
            2.0,
            0.049_787_068_367_863_944,
        ),
    ] {
        let sizes = groups.iter().map(Vec::len).collect::<Vec<_>>();
        let report =
            nonparametric::run(RankHypothesisTest::MoodMedian { groups }, &control).unwrap();
        assert!(
            (report.statistic.unwrap() - 6.0).abs() < 1e-12,
            "median categories crossed group margins: {report:?}"
        );
        assert!((report.p_value - p).abs() < 1e-12);
        assert_eq!(report.method, "mood_median");
        assert_eq!(report.null_hypothesis, "group medians are equal");
        assert_eq!(report.alternative, "two-sided");
        assert_eq!(report.statistic_name, "chi_squared");
        assert_eq!(report.degrees_of_freedom, [degrees]);
        assert_eq!(report.sample_sizes, sizes);
        assert_eq!(report.details.len(), 1);
        assert_eq!(report.details["pooled_median"], median);
        assert!(report.estimate.is_none() && report.standard_error.is_none());
    }
}
