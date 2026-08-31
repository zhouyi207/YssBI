//! Scientific-computing Tauri commands.

use crate::error::CommandError;
use crate::schema::statistics::{AcfPacfRequestDto, AcfPacfResponseDto};
use tauri::State;
use yss_application::execution::ApplicationState;
use yss_application::statistics::{
    AcfPacfApplicationError, compute_acf_pacf as compute_acf_pacf_application,
};

#[tauri::command]
pub fn compute_acf_pacf(
    application: State<ApplicationState>,
    req: AcfPacfRequestDto,
) -> Result<AcfPacfResponseDto, CommandError> {
    compute_acf_pacf_application(application.inner(), req.residuals, req.max_lag)
        .map(acf_pacf_response)
        .map_err(acf_pacf_command_error)
}

fn acf_pacf_response(
    result: yss_execution::ports::scientific::AcfPacfResult,
) -> AcfPacfResponseDto {
    AcfPacfResponseDto {
        acf: result.acf,
        pacf: result.pacf,
        n: result.n,
    }
}

fn acf_pacf_command_error(error: AcfPacfApplicationError) -> CommandError {
    match error {
        AcfPacfApplicationError::SessionCapture(error) => match error {
            yss_application::execution::SessionCaptureError::Inactive => {
                CommandError::expected("stale_project_lifecycle")
            }
            yss_application::execution::SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            yss_application::execution::SessionCaptureError::Recovering => {
                CommandError::expected("project_recovery_required")
            }
        },
        AcfPacfApplicationError::Backend(error) => match error {
            yss_execution::ports::scientific::ScientificBackendError::InvalidInput { .. } => {
                CommandError::expected("invalid_acf_pacf_input")
            }
            yss_execution::ports::scientific::ScientificBackendError::Cancelled => {
                CommandError::expected("operation_cancelled")
            }
            yss_execution::ports::scientific::ScientificBackendError::DeadlineExceeded => {
                CommandError::expected("operation_deadline_exceeded")
            }
            yss_execution::ports::scientific::ScientificBackendError::Unavailable => {
                CommandError::expected("scientific_backend_unavailable")
            }
            yss_execution::ports::scientific::ScientificBackendError::ComputationFailed => {
                CommandError::expected("scientific_computation_failed")
            }
        },
    }
}
