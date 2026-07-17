//! Serial-correlation test commands.

use crate::error::AppError;
use crate::sci::api::time_series::serial_tests::{
    SerialTestsInput, SerialTestsOutput, compute_serial_tests as compute_serial_tests_api,
};
use crate::sci::engine::SciContext;
use crate::sci::error::SciError;

#[tauri::command]
pub fn compute_serial_tests(req: SerialTestsInput) -> Result<SerialTestsOutput, AppError> {
    compute_serial_tests_api(&SciContext::rust(), req).map_err(sci_app_error)
}

fn sci_app_error(error: SciError) -> AppError {
    AppError::new(error.code(), error.to_string())
}
