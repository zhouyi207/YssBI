use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::scientific::ScientificCancellationToken;

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
    assert_eq!(result.acf, acf(&values, usize::MAX));
    assert_eq!(result.pacf, pacf(&values, usize::MAX));
    let constant = compute_acf_pacf(&[3.0; 4], 2, &control()).unwrap();
    assert_eq!(constant.acf, vec![1.0]);
    assert!(constant.pacf.is_empty());
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
