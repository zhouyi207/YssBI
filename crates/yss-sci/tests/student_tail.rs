use std::time::{Duration, Instant};
use yss_sci::hypothesis::sample_mean;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::{Alternative, ClassicalHypothesisTest, SummaryTDesign},
};

#[test]
fn sample_mean_preserves_small_and_extreme_student_tails() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    for null_mean in [-1e10, 1e10, -1e308, 1e308] {
        for alternative in [
            Alternative::TwoSided,
            Alternative::Greater,
            Alternative::Less,
        ] {
            let result = sample_mean::run(
                ClassicalHypothesisTest::OneSample {
                    values: vec![0.0, 2.0],
                    null_mean,
                    alternative,
                },
                &control,
            )
            .unwrap();
            let statistic = result.statistic.unwrap();
            assert_eq!(result.degrees_of_freedom, [1.0]);
            assert_eq!(result.standard_error, Some(1.0));
            // Student-t(1) is Cauchy: evaluate its tail without subtracting from one.
            let tail = statistic.abs().recip().atan() / std::f64::consts::PI;
            let expected = match alternative {
                Alternative::TwoSided => 2.0 * tail,
                Alternative::Greater if statistic >= 0.0 => tail,
                Alternative::Less if statistic <= 0.0 => tail,
                _ => 1.0 - tail,
            };
            assert!(
                (result.p_value / expected - 1.0).abs() < 2e-12,
                "{null_mean}, {alternative:?}: actual {}, expected {expected}",
                result.p_value
            );
        }
    }
    let welch = sample_mean::run(
        ClassicalHypothesisTest::Summary {
            design: SummaryTDesign::Independent,
            first_count: 2,
            first_mean: 1e155,
            first_sd: (2.0 + 3.0_f64.sqrt()).sqrt(),
            second_count: Some(2),
            second_mean: Some(0.0),
            second_sd: Some(1.0),
            null_difference: 0.0,
            equal_variance: false,
            alternative: Alternative::TwoSided,
        },
        &control,
    )
    .unwrap();
    assert!((welch.degrees_of_freedom[0] - 1.5).abs() < 1e-14);
    // Independently integrate the Student density at df=1.5; terms of order
    // df/t^2 are negligible here. This also checks fractional Welch degrees.
    let expected = 0.7541704864032495 * welch.statistic.unwrap().powf(-1.5);
    assert!(
        (welch.p_value / expected - 1.0).abs() < 2e-12,
        "Welch: {}, expected {expected}",
        welch.p_value
    );
    let equivalence = sample_mean::run(
        ClassicalHypothesisTest::Equivalence {
            values: vec![0.0, 2.0],
            lower_bound: -1e308,
            upper_bound: 1e308,
        },
        &control,
    )
    .unwrap();
    let expected = 1e-308 / std::f64::consts::PI;
    for actual in [
        equivalence.p_value,
        equivalence.details["p_lower_bound"],
        equivalence.details["p_upper_bound"],
    ] {
        assert!(
            (actual / expected - 1.0).abs() < 2e-12,
            "equivalence: {actual}, expected {expected}"
        );
    }
}
