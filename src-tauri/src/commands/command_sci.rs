//! Scientific-computing Tauri commands.

use crate::error::AppError;
use crate::sci::api::time_series::acf_pacf::{
    AcfPacfInput, AcfPacfOutput, compute_acf_pacf as compute_acf_pacf_api,
};
use crate::sci::engine::SciContext;
use crate::sci::error::SciError;

#[tauri::command]
pub fn compute_acf_pacf(req: AcfPacfInput) -> Result<AcfPacfOutput, AppError> {
    compute_acf_pacf_api(&SciContext::rust(), req).map_err(sci_app_error)
}

fn sci_app_error(error: SciError) -> AppError {
    AppError::new(error.code(), error.to_string())
}
