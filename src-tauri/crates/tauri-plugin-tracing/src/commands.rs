use crate::plugin::PluginState;
use serde::Serialize;
use tauri::{State, ipc::Channel};

use crate::{
    FrontendLogEntryDto, LogBatchDto, LogPage, LogQuery, LogRuntime, LogStatistics, LogStoreError,
    LogSubscriptionDto,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginError {
    code: &'static str,
    details: Option<serde_json::Value>,
    incident_id: Option<String>,
}

fn log_runtime(state: &PluginState) -> Result<LogRuntime, PluginError> {
    state
        .logs
        .clone()
        .ok_or_else(|| PluginError::new("logs_unavailable"))
}

impl PluginError {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            details: None,
            incident_id: None,
        }
    }
}

fn storage_error(error: LogStoreError) -> PluginError {
    PluginError::new(if matches!(error, LogStoreError::InvalidQuery) {
        "invalid_log_query"
    } else {
        "logs_unavailable"
    })
}

#[tauri::command]
pub fn submit_frontend_logs(
    state: State<'_, PluginState>,
    entries: Vec<FrontendLogEntryDto>,
) -> Result<(), PluginError> {
    log_runtime(&state)?
        .submit_frontend(entries)
        .map_err(|error| PluginError::new(error.code()))
}

#[tauri::command]
pub async fn subscribe_logs(
    state: State<'_, PluginState>,
    on_records: Channel<LogBatchDto>,
) -> Result<LogSubscriptionDto, PluginError> {
    let runtime = log_runtime(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        runtime.subscribe_batches(move |batch| on_records.send(batch).is_ok())
    })
    .await
    .map_err(|_| PluginError::new("logs_unavailable"))?
    .map_err(|_| PluginError::new("logs_unavailable"))
}

#[tauri::command]
pub async fn unsubscribe_logs(
    state: State<'_, PluginState>,
    subscription_id: String,
) -> Result<(), PluginError> {
    let runtime = log_runtime(&state)?;
    tauri::async_runtime::spawn_blocking(move || runtime.unsubscribe(subscription_id))
        .await
        .map_err(|_| PluginError::new("logs_unavailable"))?
        .map_err(|_| PluginError::new("logs_unavailable"))
}

#[tauri::command]
pub async fn query_logs(
    state: State<'_, PluginState>,
    query: LogQuery,
) -> Result<LogPage, PluginError> {
    let runtime = log_runtime(&state)?;
    tauri::async_runtime::spawn_blocking(move || runtime.query(query))
        .await
        .map_err(|_| PluginError::new("logs_unavailable"))?
        .map_err(storage_error)
}

#[tauri::command]
pub async fn log_statistics(state: State<'_, PluginState>) -> Result<LogStatistics, PluginError> {
    let runtime = log_runtime(&state)?;
    tauri::async_runtime::spawn_blocking(move || runtime.statistics())
        .await
        .map_err(|_| PluginError::new("logs_unavailable"))?
        .map_err(storage_error)
}
