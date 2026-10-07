use tauri::Manager;

use crate::{LogCollection, commands};

pub(crate) struct PluginState {
    pub(crate) collection: LogCollection,
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
            let collection = LogCollection::initialize(app.path().app_log_dir().ok())?;
            app.manage(PluginState { collection });
            Ok(())
        })
        .on_event(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<PluginState>().collection.shutdown();
            }
        })
        .build()
}
