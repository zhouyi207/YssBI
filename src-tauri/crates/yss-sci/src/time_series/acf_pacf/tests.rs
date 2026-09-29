use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::execution::ScientificCancellationToken;

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}

#[test]
fn combined_computation_preserves_values_and_numerical_lag_bounds() {
    let values = (0..100).map(|i| (i as f64).sin()).collect::<Vec<_>>();
    let result = compute_acf_pacf(&values, usize::MAX, &control()).unwrap();
    assert_eq!(result.n, 100);
    assert_eq!(result.acf.len(), 100);
    assert_eq!(result.acf, acf(&values, usize::MAX).unwrap());
    assert_eq!(result.pacf, pacf(&values, usize::MAX).unwrap());
    let constant = compute_acf_pacf(&[3.0; 4], 2, &control()).unwrap();
    assert_eq!(constant.acf, vec![1.0]);
    assert!(constant.pacf.is_empty());
}

#[test]
fn acf_pacf_preserves_correlations_across_extreme_finite_scales() {
    for base in [[1.0, -1.0, 1.0, -1.0], [0.5, 0.75, 1.0, 0.5]] {
        let expected = compute_acf_pacf(&base, 3, &control()).unwrap();
        for scale in [1e308, f64::MAX, f64::MIN_POSITIVE, f64::from_bits(16)] {
            let values = base.map(|value| value * scale);
            let actual = compute_acf_pacf(&values, 3, &control()).unwrap();
            for (actual, expected) in actual
                .acf
                .iter()
                .chain(&actual.pacf)
                .zip(expected.acf.iter().chain(&expected.pacf))
            {
                assert!(actual.is_finite());
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "{values:?}: {actual} != {expected}"
                );
            }
        }
    }
    let result = compute_acf_pacf(&[1e308, -1e308, 1e308, -1e308], 1, &control()).unwrap();
    assert_eq!(result.acf, vec![1.0, -0.75]);
    assert_eq!(result.pacf, vec![-0.75]);
    let constant = compute_acf_pacf(&[1e308; 4], 3, &control()).unwrap();
    assert_eq!(constant.acf, vec![1.0]);
    assert!(constant.pacf.is_empty());
}

#[test]
fn numerical_breakdown_is_an_error_instead_of_a_partial_or_fabricated_result() {
    for correlations in [
        [1.0, 1.0, 0.0],
        [1.0, 0.5, f64::NAN],
        [1.0, 0.5, f64::INFINITY],
    ] {
        assert_eq!(
            pacf_from_acf(&correlations, None),
            Err(ScientificComputationError::ComputationFailed)
        );
    }
    for result in [acf(&[1.0, f64::INFINITY], 1), pacf(&[1.0, f64::NAN], 1)] {
        assert_eq!(
            result,
            Err(ScientificComputationError::InvalidInput {
                violation: ScientificInputViolation::NonFiniteInput,
            })
        );
    }
}

#[test]
fn long_computation_observes_cancellation_and_deadline() {
    let values = (0..100_000).map(|i| (i % 71) as f64).collect::<Vec<_>>();
    let started = Instant::now();
    let control = control();
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| compute_acf_pacf(&values, 99_999, &control));
        std::thread::sleep(Duration::from_millis(20));
        control.cancellation.cancel();
        assert_eq!(
            worker.join().unwrap(),
            Err(ScientificComputationError::Cancelled)
        );
    });
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_millis(20),
    };
    assert_eq!(
        compute_acf_pacf(&values, 99_999, &control),
        Err(ScientificComputationError::DeadlineExceeded)
    );
    // Exercise the quadratic recursion separately from the ACF scan.
    let correlations = vec![0.001; 100_000];
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_millis(20),
    };
    assert_eq!(
        pacf_from_acf(&correlations, Some(&control)),
        Err(ScientificComputationError::DeadlineExceeded)
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "execution control must stop work inside the loops"
    );
}
