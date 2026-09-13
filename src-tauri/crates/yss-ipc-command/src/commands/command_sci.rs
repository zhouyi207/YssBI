//! Scientific-computing Tauri commands.

use std::time::{Duration, Instant};

use crate::error::CommandError;
use crate::schema::statistics::{AcfPacfRequestDto, AcfPacfResponseDto};
use tauri::State;
use yss_application::execution::{ApplicationState, SessionCaptureError};
use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, ScientificCancellationToken, ScientificComputationError,
    ScientificExecutionControl,
};

#[tauri::command]
pub fn compute_acf_pacf(
    application: State<ApplicationState>,
    req: AcfPacfRequestDto,
) -> Result<AcfPacfResponseDto, CommandError> {
    let _session = application.capture_session().map_err(|error| {
        CommandError::expected(match error {
            SessionCaptureError::Inactive => "stale_project_lifecycle",
            SessionCaptureError::Replacing => "project_lifecycle_admission_closed",
            SessionCaptureError::Recovering => "project_recovery_required",
        })
    })?;
    yss_sci_runtime::acf_pacf(
        AcfPacfRequest {
            values: req.residuals,
            max_lag: req.max_lag,
        },
        &ScientificExecutionControl {
            cancellation: ScientificCancellationToken::new(),
            deadline: Instant::now() + Duration::from_secs(60),
        },
    )
    .map(acf_pacf_response)
    .map_err(acf_pacf_command_error)
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
