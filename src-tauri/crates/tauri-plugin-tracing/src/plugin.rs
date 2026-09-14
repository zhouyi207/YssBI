use tauri::Manager;

use crate::{LOG_DATABASE_NAME, LogRuntime, commands};
use yss_tracing::LoggingRuntime;

pub(crate) struct PluginState {
    pub(crate) logs: Option<LogRuntime>,
}

/// Install once, before application setup, to collect logs from every Rust crate.
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("tracing")
        .invoke_handler(tauri::generate_handler![
            commands::submit_frontend_logs,
            commands::subscribe_logs,
            commands::unsubscribe_logs,
            commands::query_logs,
            commands::log_statistics,
        ])
        .setup(|app, _| {
            let logs = app.path().app_log_dir().ok()
                .and_then(|directory| LogRuntime::open(directory.join(LOG_DATABASE_NAME)).ok());
            let sink = logs.as_ref().map(LogRuntime::rust_log_sink)
                .unwrap_or_else(|| std::sync::Arc::new(|_| {}));
            let logging = LoggingRuntime::initialize(sink)?;
            app.manage(logging);
            if logs.is_none() {
                tracing::error!(target: "tauri_plugin_tracing", "Log history storage is unavailable; console logging remains enabled");
            }
            app.manage(PluginState { logs });
            Ok(())
        })
        .on_event(|app, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(state) = app.try_state::<PluginState>()
                && let Some(runtime) = &state.logs
            {
                runtime.shutdown();
            }
        })
        .build()
}
