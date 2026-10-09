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
fn goodness_of_fit_validates_frequency_totals_without_rejecting_roundoff() {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    for (observed, expected) in [
        (vec![10.0, 20.0], vec![10.0, 10.0]),
        (vec![0.0, 0.0], vec![1.0, 1.0]),
    ] {
        let result = categorical::run(Categorical::GoodnessOfFit { observed, expected }, &control);
        assert!(
            matches!(
                result,
                Err(Error::InvalidInput {
                    violation: Violation::DataOutOfRange,
                })
            ),
            "invalid frequency totals produced a test result: {result:?}"
        );
    }
    for adjustment in [0.0, 1e-7] {
        let result = categorical::run(
            Categorical::GoodnessOfFit {
                observed: vec![16.0, 18.0, 16.0, 14.0, 12.0, 12.0],
                expected: vec![16.0 + adjustment, 16.0, 16.0, 16.0, 16.0, 8.0],
            },
            &control,
        )
        .unwrap();
        assert!((result.statistic.unwrap() - 3.5).abs() < 1e-7);
        assert!((result.p_value - 0.623_387_627_749_582_2).abs() < 1e-7);
        assert_eq!(result.sample_sizes, [88]);
        assert_eq!(result.degrees_of_freedom, [5.0]);
    }
}

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
        categorical::run(
            Categorical::PearsonTable {
                observed: vec![0.0; 4],
                rows: 2,
                columns: 2,
            },
            &control,
        )
        .unwrap_err(),
        invalid(Violation::DataOutOfRange),
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
