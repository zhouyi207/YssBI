//! YssBI Tauri 组合入口。
//!
//! 入口连接 Application 运行服务与 IPC 适配器，并配置 Tauri 平台与窗口生命周期。

use tauri::Manager;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

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
        .plugin(tauri_plugin_tracing::init())
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
        .setup(yss_application::initialize)
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed)
                && let Some(application) = window.try_state::<yss_application::ApplicationState>()
            {
                application.close_result_owner(window.label());
            }
        })
        .invoke_handler(yss_application::invoke_handler())
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
                    log_domain = "ui",
                    error = %error,
                    "Failed to persist window geometry"
                );
            }
        }),
        Err(error) => tracing::error!(
            target: "yssbi::application",
            log_domain = "system",
            log_event = "applicationRuntimeFailed",
            error = %error,
            "Tauri application runtime failed"
        ),
    }
}
