use std::time::{Duration, Instant};
use yss_sci::hypothesis::{categorical, nonparametric, sample_mean, variance};
use yss_sci_contract::{
    execution::{
        ScientificCancellationToken, ScientificComputationError as Error,
        ScientificExecutionControl, ScientificInputViolation as Violation,
    },
    hypothesis::{
        Alternative, CategoricalHypothesisTest as Categorical, ClassicalHypothesisTest as Mean,
        RankHypothesisTest as Rank, VarianceHomogeneityTest as Variance,
    },
};

#[test]
fn classical_families_preserve_data_option_and_computation_errors() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let invalid = |violation| Error::InvalidInput { violation };
    assert_eq!(
        sample_mean::run(
            Mean::Paired {
                before: vec![],
                after: vec![1.0],
                alternative: Alternative::TwoSided,
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::ShapeMismatch),
    );
    assert_eq!(
        sample_mean::run(
            Mean::PoissonRate {
                counts: vec![1.0, 2.0],
                null_rate_per_observation: -1.0,
                alternative: Alternative::TwoSided,
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::ParameterOutOfRange),
    );
    assert_eq!(
        sample_mean::run(
            Mean::OneSample {
                values: vec![f64::MAX, f64::MAX],
                null_mean: 0.0,
                alternative: Alternative::TwoSided,
            },
            &control,
        )
        .unwrap_err(),
        Error::ComputationFailed,
    );
    assert_eq!(
        categorical::run(
            Categorical::GoodnessOfFit {
                observed: vec![1.0, 2.0],
                expected: vec![3.0],
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::ShapeMismatch),
    );
    assert_eq!(
        categorical::run(
            Categorical::GoodnessOfFit {
                observed: vec![1.0, f64::NAN],
                expected: vec![1.0, 1.0],
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::NonFiniteInput),
    );
    assert_eq!(
        nonparametric::run(
            Rank::Runs {
                values: vec![0.0, 2.0]
            },
            &control
        )
        .unwrap_err(),
        invalid(Violation::DataOutOfRange),
    );
    assert_eq!(
        nonparametric::run(
            Rank::WilcoxonPaired {
                before: vec![f64::MAX, 1.0],
                after: vec![-f64::MAX, 0.0],
                alternative: Alternative::TwoSided,
            },
            &control,
        )
        .unwrap_err(),
        Error::ComputationFailed,
    );
    assert_eq!(
        variance::run(
            Variance::Bartlett {
                groups: vec![vec![1.0, f64::NAN], vec![1.0, 2.0]]
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::NonFiniteInput),
    );
    assert_eq!(
        variance::run(
            Variance::Levene {
                groups: vec![vec![1.0], vec![1.0, 2.0]]
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::EmptyInput),
    );
    assert_eq!(
        variance::run(
            Variance::Bartlett {
                groups: vec![vec![f64::MAX, -f64::MAX], vec![1.0, 2.0]]
            },
            &control,
        )
        .unwrap_err(),
        Error::ComputationFailed,
    );
}
