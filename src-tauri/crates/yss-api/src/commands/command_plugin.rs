use crate::error::CommandError;
use serde_json::{Value, json};
use std::path::PathBuf;
use tauri::{State, WebviewWindow};
use yss_plugin_runtime::{
    InstalledPlugin, PackageInspection, PluginFailure, PluginManager, TaskSnapshot, ViewSession,
};

fn plugin_error(error: PluginFailure) -> CommandError {
    let code = if yss_plugin_protocol::valid_id(&error.code) {
        error.code
    } else {
        "plugin_request_failed".into()
    };
    CommandError::expected("plugin_request_failed").with_details(json!({"pluginCode":code}))
}
async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, PluginFailure> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(CommandError::internal)?
        .map_err(plugin_error)
}
#[tauri::command]
pub async fn list_plugins(
    manager: State<'_, PluginManager>,
) -> Result<Vec<InstalledPlugin>, CommandError> {
    let manager = manager.inner().clone();
    blocking(move || manager.list()).await
}
#[tauri::command]
pub async fn inspect_plugin_package(
    manager: State<'_, PluginManager>,
    path: String,
) -> Result<PackageInspection, CommandError> {
    let manager = manager.inner().clone();
    blocking(move || manager.inspect(&PathBuf::from(path))).await
}
#[tauri::command]
pub async fn install_plugin_package(
    manager: State<'_, PluginManager>,
    path: String,
    expected_digest: String,
    operation_id: String,
    approve_native: bool,
) -> Result<InstalledPlugin, CommandError> {
    let manager = manager.inner().clone();
    blocking(move || {
        manager.install(
            &PathBuf::from(path),
            &expected_digest,
            &operation_id,
            approve_native,
        )
    })
    .await
}
#[tauri::command]
pub async fn set_plugin_enabled(
    manager: State<'_, PluginManager>,
    plugin_id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    let manager = manager.inner().clone();
    blocking(move || manager.set_enabled(&plugin_id, enabled)).await
}
#[tauri::command]
pub async fn uninstall_plugin(
    manager: State<'_, PluginManager>,
    plugin_id: String,
) -> Result<(), CommandError> {
    let manager = manager.inner().clone();
    blocking(move || manager.uninstall(&plugin_id)).await
}
#[tauri::command]
pub async fn attach_plugin_view(
    window: WebviewWindow,
    manager: State<'_, PluginManager>,
    plugin_id: String,
    view_id: String,
) -> Result<ViewSession, CommandError> {
    let manager = manager.inner().clone();
    let label = window.label().to_owned();
    blocking(move || manager.attach_view(&plugin_id, &view_id, &label)).await
}
#[tauri::command]
pub async fn detach_plugin_view(
    window: WebviewWindow,
    manager: State<'_, PluginManager>,
    session_id: String,
) -> Result<(), CommandError> {
    let manager = manager.inner().clone();
    let label = window.label().to_owned();
    blocking(move || manager.detach_view(&session_id, &label)).await
}
#[tauri::command]
pub async fn call_plugin_view(
    window: WebviewWindow,
    manager: State<'_, PluginManager>,
    session_id: String,
    method: String,
    input: Value,
) -> Result<Value, CommandError> {
    let manager = manager.inner().clone();
    let label = window.label().to_owned();
    blocking(move || manager.call_view(&session_id, &label, &method, input)).await
}
#[tauri::command]
pub async fn list_plugin_tasks(
    manager: State<'_, PluginManager>,
) -> Result<Vec<TaskSnapshot>, CommandError> {
    let manager = manager.inner().clone();
    blocking(move || manager.list_tasks()).await
}
#[tauri::command]
pub fn grant_plugin_export(
    window: WebviewWindow,
    manager: State<'_, PluginManager>,
    session_id: String,
    path: String,
) -> Result<String, CommandError> {
    manager
        .grant_export(&session_id, window.label(), PathBuf::from(path))
        .map_err(plugin_error)
}
