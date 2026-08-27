use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;

use super::{
    AcfPacfRequest, AcfPacfResult, BackendCancellationToken, BackendExecutionControl, KdePoint,
    KernelDensityRequest, KernelDensityResult, ScientificBackend, ScientificBackendError,
    StatisticsOperation, StatisticsParameters, StatisticsRequest, StatisticsResult,
};
use crate::execution::settings::{
    ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
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
    statistics_calls: Mutex<Vec<(StatisticsRequest, RecordedControl)>>,
    kernel_density_calls: Mutex<Vec<(KernelDensityRequest, RecordedControl)>>,
    acf_pacf_calls: Mutex<Vec<(AcfPacfRequest, RecordedControl)>>,
}

impl RecordingScientificBackend {
    fn statistics_calls(&self) -> Vec<(StatisticsRequest, RecordedControl)> {
        self.statistics_calls
            .lock()
            .expect("recorded statistics calls lock")
            .clone()
    }

    fn kernel_density_calls(&self) -> Vec<(KernelDensityRequest, RecordedControl)> {
        self.kernel_density_calls
            .lock()
            .expect("recorded density calls lock")
            .clone()
    }

    fn acf_pacf_calls(&self) -> Vec<(AcfPacfRequest, RecordedControl)> {
        self.acf_pacf_calls
            .lock()
            .expect("recorded ACF/PACF calls lock")
            .clone()
    }
}

impl ScientificBackend for RecordingScientificBackend {
    fn statistics(
        &self,
        request: StatisticsRequest,
        control: &BackendExecutionControl,
    ) -> Result<StatisticsResult, ScientificBackendError> {
        self.statistics_calls
            .lock()
            .expect("recorded statistics calls lock")
            .push((request, RecordedControl::from(control)));
        Ok(StatisticsResult {
            value: json!({ "family": "fake" }),
        })
    }

    fn kernel_density(
        &self,
        request: KernelDensityRequest,
        control: &BackendExecutionControl,
    ) -> Result<KernelDensityResult, ScientificBackendError> {
        self.kernel_density_calls
            .lock()
            .expect("recorded density calls lock")
            .push((request, RecordedControl::from(control)));
        Ok(KernelDensityResult {
            points: vec![KdePoint {
                x: 1.0,
                density: 0.25,
            }],
        })
    }

    fn acf_pacf(
        &self,
        request: AcfPacfRequest,
        control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError> {
        let n = request.values.len();
        self.acf_pacf_calls
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

fn settings() -> ExecutionSettings {
    ExecutionSettings {
        numeric_tolerance: ExecutionNumericTolerance {
            absolute: 1e-7,
            relative: 1e-5,
        },
        statistical_missing_values: ExecutionMissingValuePolicy::Reject,
    }
}

#[test]
fn dynamic_backend_keeps_each_request_bound_to_its_result_family_and_control() {
    let backend = RecordingScientificBackend::default();
    let dynamic_backend: &dyn ScientificBackend = &backend;
    let cancellation = BackendCancellationToken::new();
    cancellation.cancel();
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = BackendExecutionControl {
        cancellation,
        deadline,
    };
    let statistics_request = StatisticsRequest {
        operation: StatisticsOperation::AugmentedDickeyFuller,
        parameters: StatisticsParameters::default(),
        inputs: vec![vec![1.0, 0.5, 0.25, 0.125]],
        settings: settings(),
    };
    let density_request = KernelDensityRequest {
        values: vec![0.0, 1.0],
        grid_points: 8,
        min_x: None,
    };
    let acf_pacf_request = AcfPacfRequest {
        values: vec![1.0, 0.0, -1.0, 0.0],
        max_lag: 1,
    };

    let statistics: StatisticsResult = dynamic_backend
        .statistics(statistics_request.clone(), &control)
        .expect("the fake statistics family must return its typed result");
    let density: KernelDensityResult = dynamic_backend
        .kernel_density(density_request.clone(), &control)
        .expect("the fake density family must return its typed result");
    let acf_pacf: AcfPacfResult = dynamic_backend
        .acf_pacf(acf_pacf_request.clone(), &control)
        .expect("the fake ACF/PACF family must return its typed result");

    assert_eq!(statistics.value, json!({ "family": "fake" }));
    assert_eq!(density.points[0].density, 0.25);
    assert_eq!(acf_pacf.n, 4);
    let recorded_control = RecordedControl {
        cancelled: true,
        deadline,
    };
    assert_eq!(
        backend.statistics_calls(),
        vec![(statistics_request, recorded_control)]
    );
    assert_eq!(
        backend.kernel_density_calls(),
        vec![(density_request, recorded_control)]
    );
    assert_eq!(
        backend.acf_pacf_calls(),
        vec![(acf_pacf_request, recorded_control)]
    );
}
