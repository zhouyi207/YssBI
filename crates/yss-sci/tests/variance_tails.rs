use std::time::{Duration, Instant};
use yss_sci::hypothesis::variance;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::VarianceHomogeneityTest,
};

#[test]
fn variance_homogeneity_retains_small_upper_tail_probabilities() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let groups = vec![
        vec![-1.125, -0.875, 0.875, 1.125],
        vec![
            -1_000_000_000_000.125,
            -999_999_999_999.875,
            999_999_999_999.875,
            1_000_000_000_000.125,
        ],
    ];
    let reports = [
        VarianceHomogeneityTest::Levene {
            groups: groups.clone(),
        },
        VarianceHomogeneityTest::BrownForsythe { groups },
        VarianceHomogeneityTest::Bartlett {
            groups: vec![vec![1.0, 2.0, 3.0, 4.0], vec![1e8, 2e8, 3e8, 4e8]],
        },
    ]
    .map(|input| variance::run(input, &control).unwrap());
    for (report, method, statistic, degrees, p) in [
        (
            &reports[0],
            "levene",
            9.599_999_999_980_8e25,
            vec![1.0, 6.0],
            7.629_394_531_295_777e-77,
        ),
        (
            &reports[1],
            "brown_forsythe",
            9.599_999_999_980_8e25,
            vec![1.0, 6.0],
            7.629_394_531_295_777e-77,
        ),
        (
            &reports[2],
            "bartlett",
            91.170_172_611_732_43,
            vec![1.0],
            1.318_325_745_654_562_2e-21,
        ),
    ] {
        assert!((report.statistic.unwrap() / statistic - 1.0).abs() < 1e-12);
        assert!(
            (report.p_value / p - 1.0).abs() < 1e-12,
            "representable variance-test tails were lost: {reports:?}"
        );
        assert_eq!(report.method, method);
        assert_eq!(report.degrees_of_freedom, degrees);
        assert_eq!(report.sample_sizes, [4, 4]);
    }

    let groups = vec![vec![-1.125, -0.875, 0.875, 1.125]; 2];
    for input in [
        VarianceHomogeneityTest::Levene {
            groups: groups.clone(),
        },
        VarianceHomogeneityTest::BrownForsythe { groups },
    ] {
        let report = variance::run(input, &control).unwrap();
        assert_eq!(report.statistic, Some(0.0));
        assert_eq!(report.p_value, 1.0);
    }
}
