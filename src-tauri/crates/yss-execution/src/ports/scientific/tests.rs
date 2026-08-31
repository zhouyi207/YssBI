use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{
    AcfPacfRequest, AcfPacfResult, BackendCancellationToken, BackendExecutionControl,
    ScientificBackend, ScientificBackendError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordedControl {
    cancelled: bool,
    deadline: Instant,
}

impl RecordedControl {
    fn from(control: &BackendExecutionControl) -> Self {
        Self {
            cancelled: control.cancellation.is_cancelled(),
            deadline: control.deadline,
        }
    }
}

#[derive(Default)]
struct RecordingScientificBackend {
    calls: Mutex<Vec<(AcfPacfRequest, RecordedControl)>>,
}

impl ScientificBackend for RecordingScientificBackend {
    fn acf_pacf(
        &self,
        request: AcfPacfRequest,
        control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError> {
        let n = request.values.len();
        self.calls
            .lock()
            .expect("recorded ACF/PACF calls lock")
            .push((request, RecordedControl::from(control)));
        Ok(AcfPacfResult {
            acf: vec![1.0, 0.5],
            pacf: vec![0.5],
            n,
        })
    }
}

#[test]
fn dynamic_backend_preserves_acf_pacf_request_result_and_control() {
    let backend = RecordingScientificBackend::default();
    let dynamic_backend: &dyn ScientificBackend = &backend;
    let cancellation = BackendCancellationToken::new();
    cancellation.cancel();
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = BackendExecutionControl {
        cancellation,
        deadline,
    };
    let request = AcfPacfRequest {
        values: vec![1.0, 0.0, -1.0, 0.0],
        max_lag: 1,
    };

    let result: AcfPacfResult = dynamic_backend
        .acf_pacf(request.clone(), &control)
        .expect("the fake ACF/PACF backend must return its typed result");

    assert_eq!(result.n, 4);
    assert_eq!(
        *backend.calls.lock().expect("recorded calls lock"),
        vec![(
            request,
            RecordedControl {
                cancelled: true,
                deadline,
            },
        )]
    );
}
