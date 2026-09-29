use std::time::{Duration, Instant};
use yss_sci_contract::execution::{ScientificCancellationToken, ScientificExecutionControl};

fn active_control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}

use super::acf_pacf;
use crate::error::map_sci_error;
use yss_sci_contract::execution::{ScientificComputationError, ScientificInputViolation};
use yss_sci_contract::time_series::acf_pacf::AcfPacfRequest;
use yss_sci_contract::{SciError, SciOperationCode};

#[test]
fn acf_pacf_maps_results_and_rejects_invalid_requests() {
    let result = acf_pacf(
        AcfPacfRequest {
            values: vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0],
            max_lag: 2,
        },
        &active_control(),
    )
    .expect("a valid ACF/PACF request must map through the SCI runtime");
    assert_eq!(result.acf.len(), 3);
    assert_eq!(result.pacf.len(), 2);
    assert_eq!(result.n, 6);
    let bounded = acf_pacf(
        AcfPacfRequest {
            values: (0..200).map(|value| (value % 13) as f64).collect(),
            max_lag: usize::MAX,
        },
        &active_control(),
    )
    .unwrap();
    assert_eq!(bounded.acf.len(), 41);
    assert_eq!(bounded.pacf.len(), 40);

    assert_eq!(
        acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, f64::NAN, -1.0, 0.0],
                max_lag: 1,
            },
            &active_control(),
        ),
        Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::NonFiniteInput,
        })
    );
    assert_eq!(
        acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0],
                max_lag: 0,
            },
            &active_control(),
        ),
        Err(ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        })
    );
    assert_eq!(
        map_sci_error(SciError::ComputationFailed {
            operation: SciOperationCode::AcfPacf,
        }),
        ScientificComputationError::ComputationFailed
    );
}

#[test]
fn computations_reject_cancelled_or_expired_calls_at_admission() {
    let request = || AcfPacfRequest {
        values: vec![1.0, 0.0, -1.0, 0.0],
        max_lag: 1,
    };
    let cancelled = ScientificCancellationToken::new();
    cancelled.cancel();

    assert_eq!(
        acf_pacf(
            request(),
            &ScientificExecutionControl {
                cancellation: cancelled,
                deadline: Instant::now() + Duration::from_secs(5),
            },
        ),
        Err(ScientificComputationError::Cancelled)
    );
    assert_eq!(
        acf_pacf(
            request(),
            &ScientificExecutionControl {
                cancellation: ScientificCancellationToken::new(),
                deadline: Instant::now(),
            },
        ),
        Err(ScientificComputationError::DeadlineExceeded)
    );
}

#[test]
fn acf_pacf_rejects_nonfinite_results_at_the_runtime_boundary() {
    use yss_sci_contract::time_series::acf_pacf::AcfPacfResult;
    for result in [
        AcfPacfResult {
            acf: vec![1.0, f64::NAN],
            pacf: vec![0.5],
            n: 4,
        },
        AcfPacfResult {
            acf: vec![1.0, 0.5],
            pacf: vec![f64::INFINITY],
            n: 4,
        },
    ] {
        assert_eq!(
            super::validate_result(&result),
            Err(ScientificComputationError::ComputationFailed)
        );
    }
}
