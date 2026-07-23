//! Tauri commands for the system Julia runtime.

use tauri::{Manager, State, async_runtime};

use crate::error::AppError;
use crate::julia::worker::{JuliaWorkerManager, JuliaWorkerStatus};
use crate::julia::{JuliaRuntimeStatus, get_runtime_status, install_latest_julia};

/// Returns the status of the Julia executable available to the operating system.
#[tauri::command]
pub async fn get_julia_runtime_status() -> Result<JuliaRuntimeStatus, AppError> {
    async_runtime::spawn_blocking(|| Ok(get_runtime_status()))
        .await
        .map_err(AppError::internal)?
}

#[tauri::command]
pub async fn get_julia_worker_status(
    app: tauri::AppHandle,
    worker: State<'_, JuliaWorkerManager>,
) -> Result<JuliaWorkerStatus, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(AppError::internal)?;
    let worker = worker.inner().clone();
    async_runtime::spawn_blocking(move || Ok(worker.status(&app_data_dir)))
        .await
        .map_err(AppError::internal)?
}

/// Installs the latest Julia release when Julia is not available on the system.
#[tauri::command]
pub async fn install_julia_runtime(
    app: tauri::AppHandle,
    worker: State<'_, JuliaWorkerManager>,
) -> Result<JuliaRuntimeStatus, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(AppError::internal)?;
    let worker = worker.inner().clone();
    async_runtime::spawn_blocking(move || {
        let status =
            install_latest_julia().map_err(|error| AppError::new("julia_install_failed", error))?;
        worker
            .warm_up(&app_data_dir)
            .map_err(|error| AppError::new("julia_worker_start_failed", error))?;
        Ok(status)
    })
    .await
    .map_err(AppError::internal)?
}
