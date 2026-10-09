use std::time::{Duration, Instant};
use yss_sci::hypothesis::sample_mean;
use yss_sci_contract::{
    execution::{
        ScientificCancellationToken, ScientificComputationError, ScientificExecutionControl,
    },
    hypothesis::{Alternative, ClassicalHypothesisTest},
};

#[test]
fn discrete_count_admission_uses_execution_control_instead_of_fixed_sample_caps() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    for (successes, probability, alternative) in [
        (0, 0.5, Alternative::Greater),
        (1_000_001, 0.5, Alternative::Less),
        (0, 0.0, Alternative::TwoSided),
        (1_000_001, 1.0, Alternative::TwoSided),
    ] {
        let report = sample_mean::run(
            ClassicalHypothesisTest::ExactBinomial {
                successes,
                trials: 1_000_001,
                null_probability: probability,
                alternative,
            },
            &control,
        )
        .unwrap();
        assert_eq!(report.p_value, 1.0);
    }
    for (count, rate, alternative) in [
        (0.0, 500_001.0, Alternative::Greater),
        (1_000_001.0, 0.0, Alternative::Less),
        (1_000_001.0, 1.0, Alternative::Less),
    ] {
        let report = sample_mean::run(
            ClassicalHypothesisTest::PoissonRate {
                counts: vec![count],
                null_rate_per_observation: rate,
                alternative,
            },
            &control,
        )
        .unwrap();
        assert_eq!(report.p_value, 1.0);
    }
    for input in [
        ClassicalHypothesisTest::ExactBinomial {
            successes: 50_000_000,
            trials: 100_000_000,
            null_probability: 0.5,
            alternative: Alternative::TwoSided,
        },
        ClassicalHypothesisTest::PoissonRate {
            counts: vec![1.0],
            null_rate_per_observation: 1_000_000.0,
            alternative: Alternative::TwoSided,
        },
    ] {
        let timed = ScientificExecutionControl {
            cancellation: ScientificCancellationToken::new(),
            deadline: Instant::now() + Duration::from_millis(10),
        };
        assert_eq!(
            sample_mean::run(input, &timed).unwrap_err(),
            ScientificComputationError::DeadlineExceeded,
        );
    }
}
