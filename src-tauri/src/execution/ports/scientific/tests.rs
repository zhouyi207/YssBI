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

struct FakeScientificBackend;

impl ScientificBackend for FakeScientificBackend {
    fn statistics(
        &self,
        _request: StatisticsRequest,
        control: &BackendExecutionControl,
    ) -> Result<StatisticsResult, ScientificBackendError> {
        if control.cancellation.is_cancelled() {
            return Err(ScientificBackendError::Cancelled);
        }
        Ok(StatisticsResult {
            value: json!({ "family": "fake" }),
        })
    }

    fn kernel_density(
        &self,
        _request: KernelDensityRequest,
        _control: &BackendExecutionControl,
    ) -> Result<KernelDensityResult, ScientificBackendError> {
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
        _control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError> {
        Ok(AcfPacfResult {
            acf: vec![1.0, 0.5],
            pacf: vec![0.5],
            n: request.values.len(),
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
    let backend: &dyn ScientificBackend = &FakeScientificBackend;
    let cancellation = BackendCancellationToken::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = BackendExecutionControl {
        cancellation: cancellation.clone(),
        deadline,
    };

    let statistics: StatisticsResult = backend
        .statistics(
            StatisticsRequest {
                operation: StatisticsOperation::AugmentedDickeyFuller,
                parameters: StatisticsParameters::default(),
                inputs: vec![vec![1.0, 0.5, 0.25, 0.125]],
                settings: settings(),
            },
            &control,
        )
        .expect("the fake statistics family must return its typed result");
    let density: KernelDensityResult = backend
        .kernel_density(
            KernelDensityRequest {
                values: vec![0.0, 1.0],
                grid_points: 8,
                min_x: None,
            },
            &control,
        )
        .expect("the fake density family must return its typed result");
    let acf_pacf: AcfPacfResult = backend
        .acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0],
                max_lag: 1,
            },
            &control,
        )
        .expect("the fake ACF/PACF family must return its typed result");

    assert_eq!(statistics.value, json!({ "family": "fake" }));
    assert_eq!(density.points[0].density, 0.25);
    assert_eq!(acf_pacf.n, 4);
    assert_eq!(control.deadline, deadline);

    cancellation.cancel();
    assert_eq!(
        backend.statistics(
            StatisticsRequest {
                operation: StatisticsOperation::AugmentedDickeyFuller,
                parameters: StatisticsParameters::default(),
                inputs: vec![vec![1.0, 0.5, 0.25, 0.125]],
                settings: settings(),
            },
            &control,
        ),
        Err(ScientificBackendError::Cancelled)
    );
}
