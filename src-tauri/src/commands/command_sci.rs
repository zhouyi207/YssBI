//! Scientific-computing Tauri commands.

use crate::error::CommandError;
use crate::sci::api::time_series::acf_pacf::{
    AcfPacfInput, AcfPacfOutput, compute_acf_pacf as compute_acf_pacf_api,
};
use crate::sci::engine::SciContext;
use crate::sci::error::SciError;

#[tauri::command]
pub fn compute_acf_pacf(req: AcfPacfInput) -> Result<AcfPacfOutput, CommandError> {
    compute_acf_pacf_api(&SciContext::rust(), req).map_err(sci_command_error)
}

fn sci_command_error(error: SciError) -> CommandError {
    CommandError::expected(error.code())
}
