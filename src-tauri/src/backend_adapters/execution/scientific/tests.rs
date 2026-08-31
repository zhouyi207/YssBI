use std::time::{Duration, Instant};

use super::{SciApiScientificBackend, map_sci_error, map_settings};
use yss_execution::ports::scientific::{
    AcfPacfRequest, BackendCancellationToken, BackendExecutionControl, ExecutionRegressionKind,
    KdePoint, KernelDensityRequest, ScientificBackend, ScientificBackendError,
    ScientificInputViolation, StatisticsOperation, StatisticsParameters, StatisticsRequest,
};
use yss_execution::settings::{
    ExecutionMissingValuePolicy, ExecutionNumericTolerance, ExecutionSettings,
};
use yss_sci_contract::{
    MissingValuePolicy, NumericTolerance, SciComputationSettings, SciError, SciOperationCode,
};

fn settings(missing_values: ExecutionMissingValuePolicy) -> ExecutionSettings {
    ExecutionSettings {
        numeric_tolerance: ExecutionNumericTolerance {
            absolute: 1e-7,
            relative: 1e-5,
        },
        statistical_missing_values: missing_values,
    }
}

fn active_control() -> BackendExecutionControl {
    BackendExecutionControl {
        cancellation: BackendCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(5),
    }
}

#[test]
fn adapter_maps_settings_requests_results_and_closed_errors() {
    let backend: &dyn ScientificBackend = &SciApiScientificBackend::new();
    assert_eq!(
        map_settings(settings(ExecutionMissingValuePolicy::Listwise)),
        Ok(SciComputationSettings {
            tolerance: NumericTolerance {
                absolute: 1e-7,
                relative: 1e-5,
            },
            missing_values: MissingValuePolicy::Listwise,
        })
    );
    assert_eq!(
        map_settings(settings(ExecutionMissingValuePolicy::Reject))
            .expect("the second closed settings variant must map")
            .missing_values,
        MissingValuePolicy::Reject
    );

    let statistics = backend
        .statistics(
            StatisticsRequest {
                operation: StatisticsOperation::Regression {
                    kind: ExecutionRegressionKind::Ols,
                },
                parameters: StatisticsParameters::default(),
                inputs: vec![vec![1.0, 2.0, 3.0, 4.0], vec![0.0, 1.0, 2.0, 3.0]],
                settings: settings(ExecutionMissingValuePolicy::Reject),
            },
            &active_control(),
        )
        .expect("a valid regression request must map through the SCI API");
    assert_eq!(statistics.value["family"], "ols");
    assert_eq!(statistics.value["metadata"]["missingValuePolicy"], "reject");
    assert_eq!(
        statistics.value["metadata"]["effectiveConvergenceTolerance"],
        1e-7
    );

    let density = backend
        .kernel_density(
            KernelDensityRequest {
                values: vec![0.0, 1.0, 2.0],
                grid_points: 8,
                min_x: Some(0.0),
            },
            &active_control(),
        )
        .expect("a valid KDE request must map through the SCI API");
    assert_eq!(density.points.len(), 8);
    assert!(density.points.windows(2).all(|pair| pair[0].x < pair[1].x));
    let _: &KdePoint = &density.points[0];

    let acf_pacf = backend
        .acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0],
                max_lag: 2,
            },
            &active_control(),
        )
        .expect("a valid ACF/PACF request must map through the SCI API");
    assert_eq!(acf_pacf.acf.len(), 3);
    assert_eq!(acf_pacf.pacf.len(), 2);
    assert_eq!(acf_pacf.n, 6);

    let non_finite = backend.statistics(
        StatisticsRequest {
            operation: StatisticsOperation::Regression {
                kind: ExecutionRegressionKind::Ols,
            },
            parameters: StatisticsParameters::default(),
            inputs: vec![vec![1.0, f64::NAN], vec![0.0, 1.0]],
            settings: settings(ExecutionMissingValuePolicy::Listwise),
        },
        &active_control(),
    );
    assert_eq!(
        non_finite,
        Err(ScientificBackendError::InvalidInput {
            violation: ScientificInputViolation::NonFiniteInput,
        })
    );

    let invalid_parameter = backend.kernel_density(
        KernelDensityRequest {
            values: vec![0.0, 1.0],
            grid_points: 1,
            min_x: None,
        },
        &active_control(),
    );
    assert_eq!(
        invalid_parameter,
        Err(ScientificBackendError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        })
    );

    assert_eq!(
        map_sci_error(SciError::ComputationFailed {
            operation: SciOperationCode::Regression,
        }),
        ScientificBackendError::ComputationFailed
    );
}

#[test]
fn adapter_rejects_cancelled_or_expired_calls_at_admission() {
    let backend: &dyn ScientificBackend = &SciApiScientificBackend::new();
    let cancelled = BackendCancellationToken::new();
    cancelled.cancel();

    assert_eq!(
        backend.kernel_density(
            KernelDensityRequest {
                values: vec![0.0, 1.0],
                grid_points: 8,
                min_x: None,
            },
            &BackendExecutionControl {
                cancellation: cancelled,
                deadline: Instant::now() + Duration::from_secs(5),
            },
        ),
        Err(ScientificBackendError::Cancelled)
    );
    assert_eq!(
        backend.acf_pacf(
            AcfPacfRequest {
                values: vec![1.0, 0.0, -1.0, 0.0],
                max_lag: 1,
            },
            &BackendExecutionControl {
                cancellation: BackendCancellationToken::new(),
                deadline: Instant::now(),
            },
        ),
        Err(ScientificBackendError::DeadlineExceeded)
    );
}
