//! Scientific-computing Tauri commands.

use std::time::{Duration, Instant};

use crate::ipc::error::CommandError;
use crate::ipc::schema::statistics::{AcfPacfRequestDto, AcfPacfResponseDto};
use crate::session::{ApplicationState, SessionCaptureError};
use tauri::State;
use yss_sci_contract::execution::{
    ScientificCancellationToken, ScientificComputationError, ScientificExecutionControl,
};
use yss_sci_contract::time_series::acf_pacf::{AcfPacfRequest, AcfPacfResult};

#[tauri::command]
pub async fn compute_acf_pacf(
    application: State<'_, ApplicationState>,
    req: AcfPacfRequestDto,
) -> Result<AcfPacfResponseDto, CommandError> {
    let session = application.capture_session().map_err(|error| {
        CommandError::expected(match error {
            SessionCaptureError::Inactive => "stale_project_lifecycle",
            SessionCaptureError::Replacing => "project_lifecycle_admission_closed",
            SessionCaptureError::Recovering => "project_recovery_required",
        })
    })?;
    // Queue time consumes the same budget as numerical work.
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    };
    let worker_control = control.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        yss_sci_runtime::time_series::acf_pacf(
            AcfPacfRequest {
                values: req.residuals,
                max_lag: req.max_lag,
            },
            &worker_control,
        )
    })
    .await
    .map_err(CommandError::internal)?
    .map_err(acf_pacf_command_error)?;
    application
        .revalidate_captured_session(&session)
        .map_err(|_| CommandError::expected("stale_project_lifecycle"))?;
    control.check().map_err(acf_pacf_command_error)?;
    Ok(acf_pacf_response(result))
}

fn acf_pacf_response(result: AcfPacfResult) -> AcfPacfResponseDto {
    AcfPacfResponseDto {
        acf: result.acf,
        pacf: result.pacf,
        n: result.n,
    }
}

fn acf_pacf_command_error(error: ScientificComputationError) -> CommandError {
    CommandError::expected(match error {
        ScientificComputationError::InvalidInput { .. } => "invalid_acf_pacf_input",
        ScientificComputationError::Cancelled => "operation_cancelled",
        ScientificComputationError::DeadlineExceeded => "operation_deadline_exceeded",
        ScientificComputationError::ComputationFailed => "scientific_computation_failed",
    })
}
