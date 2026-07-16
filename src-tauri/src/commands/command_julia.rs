//! Tauri commands for the system Julia installation.

use tauri::async_runtime;

use crate::error::AppError;
use crate::julia::{JuliaRuntimeStatus, get_runtime_status, install_latest_julia};

/// Returns the status of the Julia executable available to the operating system.
#[tauri::command]
pub async fn get_julia_runtime_status() -> Result<JuliaRuntimeStatus, AppError> {
    async_runtime::spawn_blocking(|| Ok(get_runtime_status()))
        .await
        .map_err(AppError::internal)?
}

/// Installs the latest Julia release when Julia is not available on the system.
#[tauri::command]
pub async fn install_julia_runtime() -> Result<JuliaRuntimeStatus, AppError> {
    async_runtime::spawn_blocking(|| {
        install_latest_julia().map_err(|error| AppError::new("julia_install_failed", error))
    })
    .await
    .map_err(AppError::internal)?
}
