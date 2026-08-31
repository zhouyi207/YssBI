use std::time::{Duration, Instant};

use super::{SciRuntimeBackend, map_sci_error};
use yss_execution::ports::scientific::{
    AcfPacfRequest, BackendCancellationToken, BackendExecutionControl, ScientificBackend,
    ScientificBackendError, ScientificInputViolation,
};
use yss_sci_contract::{SciError, SciOperationCode};

fn active_control() -> BackendExecutionControl {
    BackendExecutionControl {
        cancellation: BackendCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}

#[test]
fn adapter_maps_acf_pacf_results_and_rejects_invalid_requests() {
    let backend: &dyn ScientificBackend = &SciRuntimeBackend::new();
    let result = backend
        .acf_pacf(
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

    assert_eq!(
        backend.acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, f64::NAN, -1.0, 0.0],
                max_lag: 1,
            },
            &active_control(),
        ),
        Err(ScientificBackendError::InvalidInput {
            violation: ScientificInputViolation::NonFiniteInput,
        })
    );
    assert_eq!(
        backend.acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0],
                max_lag: 0,
            },
            &active_control(),
        ),
        Err(ScientificBackendError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        })
    );
    assert_eq!(
        map_sci_error(SciError::ComputationFailed {
            operation: SciOperationCode::AcfPacf,
        }),
        ScientificBackendError::ComputationFailed
    );
}

#[test]
fn adapter_rejects_cancelled_or_expired_calls_at_admission() {
    let backend: &dyn ScientificBackend = &SciRuntimeBackend::new();
    let request = || AcfPacfRequest {
        values: vec![1.0, 0.0, -1.0, 0.0],
        max_lag: 1,
    };
    let cancelled = BackendCancellationToken::new();
    cancelled.cancel();

    assert_eq!(
        backend.acf_pacf(
            request(),
            &BackendExecutionControl {
                cancellation: cancelled,
                deadline: Instant::now() + Duration::from_secs(5),
            },
        ),
        Err(ScientificBackendError::Cancelled)
    );
    assert_eq!(
        backend.acf_pacf(
            request(),
            &BackendExecutionControl {
                cancellation: BackendCancellationToken::new(),
                deadline: Instant::now(),
            },
        ),
        Err(ScientificBackendError::DeadlineExceeded)
    );
}
