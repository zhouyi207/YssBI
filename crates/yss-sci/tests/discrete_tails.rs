use std::time::{Duration, Instant};
use yss_sci::hypothesis::sample_mean;
use yss_sci_contract::{
    execution::{ScientificCancellationToken, ScientificExecutionControl},
    hypothesis::{Alternative, ClassicalHypothesisTest},
};

#[test]
fn exact_discrete_tests_include_zero_and_respect_degenerate_tail_direction() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let alternatives = [
        Alternative::Greater,
        Alternative::Less,
        Alternative::TwoSided,
    ];
    for (successes, probability, expected) in [
        (0, 0.5, [1.0, 0.25, 0.5]),
        (1, 0.5, [0.75, 0.75, 1.0]),
        (2, 0.5, [0.25, 1.0, 0.5]),
        (0, 0.0, [1.0, 1.0, 1.0]),
        (1, 0.0, [0.0, 1.0, 0.0]),
        (2, 0.0, [0.0, 1.0, 0.0]),
        (0, 1.0, [1.0, 0.0, 0.0]),
        (1, 1.0, [1.0, 0.0, 0.0]),
        (2, 1.0, [1.0, 1.0, 1.0]),
    ] {
        for (alternative, expected) in alternatives.into_iter().zip(expected) {
            let report = sample_mean::run(
                ClassicalHypothesisTest::ExactBinomial {
                    successes,
                    trials: 2,
                    null_probability: probability,
                    alternative,
                },
                &control,
            )
            .unwrap();
            assert!(
                (report.p_value - expected).abs() < 1e-12,
                "binomial successes={successes}, p={probability}, alternative={alternative:?}: {} != {expected}",
                report.p_value,
            );
        }
    }
    let zero_mass = (-2.0_f64).exp();
    for (counts, rate, expected) in [
        (
            [0.0, 0.0],
            1.0,
            [1.0, zero_mass, 1.0 - 16.0 / 3.0 * zero_mass],
        ),
        ([1.0, 0.0], 1.0, [1.0 - zero_mass, 3.0 * zero_mass, 1.0]),
        ([0.0, 0.0], 0.0, [1.0, 1.0, 1.0]),
        ([1.0, 0.0], 0.0, [0.0, 1.0, 0.0]),
    ] {
        for (alternative, expected) in alternatives.into_iter().zip(expected) {
            let report = sample_mean::run(
                ClassicalHypothesisTest::PoissonRate {
                    counts: counts.to_vec(),
                    null_rate_per_observation: rate,
                    alternative,
                },
                &control,
            )
            .unwrap();
            assert!(
                (report.p_value - expected).abs() < 1e-12,
                "Poisson counts={counts:?}, rate={rate}, alternative={alternative:?}: {} != {expected}",
                report.p_value,
            );
        }
    }
}
