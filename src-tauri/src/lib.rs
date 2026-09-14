//! YssBI Tauri 组合入口。
//!
//! 这里只构造并注入各 crate authority、Application state 与平台适配器；领域行为和
//! transport contract 分别留在各自 owner。

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod architecture_tests;

use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
mod harness;
mod projects;

// ==================== 应用入口 ====================

const WINDOW_STATE_FLAGS: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

fn window_state_key(label: &str) -> &str {
    label.split_once('-').map_or(label, |(kind, _)| kind)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(WINDOW_STATE_FLAGS)
                .map_label(window_state_key)
                .build(),
        )
        // 注册全局状态管理器
        .manage(projects::watcher())
        .manage(yss_ipc_command::ActivityPanelSyncState::default())
        .setup(move |app| {
            let log_dir = app.path().app_log_dir();
            let diagnostics = yss_diagnostics::DiagnosticsRuntime::initialize()
                .map_err(Box::<dyn std::error::Error>::from)?;
            let logging = yss_tracing::LoggingRuntime::initialize(
                log_dir.as_ref().ok().cloned(),
                Some(diagnostics.rust_log_sink()),
            )
            .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(logging);
            app.manage(diagnostics);
            if let Err(error) = log_dir {
                tracing::error!(
                    target: "yssbi::logging",
                    diagnostic_domain = "system",
                    diagnostic_event = "appLogDirectoryUnavailable",
                    error = %error,
                    "Failed to resolve application log directory; file logging is disabled"
                );
            }

            let application_state = yss_application::execution::ApplicationState::initialize()
                .map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(application_state.clone());
            app.manage(yss_application::database::samples::SampleCatalog::new(
                app.path()
                    .resolve("resources/samples", tauri::path::BaseDirectory::Resource)?,
            ));

            let app_dir = app.path().app_data_dir()?;
            let harness_state = tauri::async_runtime::block_on(harness::initialize(
                app_dir.clone(),
                application_state.clone(),
            ))?;
            app.manage(harness_state);
            app.manage(tauri::async_runtime::block_on(
                projects::initialize_registry(app_dir.clone()),
            )?);
            let plugins = yss_plugin_runtime::PluginManager::initialize(
                &app_dir,
                Arc::new(yss_application::plugins::PluginHostServices::new(
                    application_state,
                )),
            );
            app.manage(plugins);

            // Configured windows run the plugin's restore hook before application setup.
            if let Some(win) = app.get_webview_window("main") {
                if let Err(error) = win.show() {
                    tracing::warn!(
                        target: "yssbi::window_state",
                        diagnostic_domain = "ui",
                        error = %error,
                        "Failed to show main window"
                    );
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && let Some(application) =
                    window.try_state::<yss_application::execution::ApplicationState>()
            {
                application.close_result_owner(window.label());
            }
        })
        .invoke_handler(yss_ipc_command::invoke_handler())
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|app, event| {
            // The manager has removed the destroyed window before this callback. Saving
            // here cannot query a dead window or participate in frontend close decisions.
            if matches!(
                event,
                tauri::RunEvent::WindowEvent {
                    event: tauri::WindowEvent::Destroyed,
                    ..
                }
            ) && let Err(error) = app.save_window_state(WINDOW_STATE_FLAGS)
            {
                tracing::warn!(
                    target: "yssbi::window_state",
                    diagnostic_domain = "ui",
                    error = %error,
                    "Failed to persist window geometry"
                );
            }
        }),
        Err(error) => tracing::error!(
            target: "yssbi::application",
            diagnostic_domain = "system",
            diagnostic_event = "applicationRuntimeFailed",
            error = %error,
            "Tauri application runtime failed"
        ),
    }
}
