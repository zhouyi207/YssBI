use tauri::Manager;

use crate::collector::LoggingRuntime;
use crate::{LOG_DATABASE_NAME, LogRuntime, commands};

pub(crate) struct PluginState {
    pub(crate) logs: Option<LogRuntime>,
    runtime: LogRuntime,
    _logging: LoggingRuntime,
}

impl Drop for PluginState {
    fn drop(&mut self) {
        // Drain storage while the collector's output workers are still available.
        self.runtime.shutdown();
    }
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
            let runtime = match &logs {
                Some(logs) => logs.clone(),
                None => LogRuntime::initialize()?,
            };
            let logging = LoggingRuntime::initialize(|console| runtime.rust_log_sink(Some(console)))?;
            if logs.is_none() {
                tracing::error!(target: "tauri_plugin_tracing", "Log history storage is unavailable; console logging remains enabled");
            }
            app.manage(PluginState { logs, runtime, _logging: logging });
            Ok(())
        })
        .on_event(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<PluginState>().runtime.shutdown();
            }
        })
        .build()
}
