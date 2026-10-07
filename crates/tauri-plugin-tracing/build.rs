#[cfg(feature = "tauri-host")]
const COMMANDS: &[&str] = &[
    "submit_frontend_logs",
    "subscribe_logs",
    "unsubscribe_logs",
    "query_logs",
    "log_statistics",
];

fn main() {
    #[cfg(feature = "tauri-host")]
    tauri_plugin::Builder::new(COMMANDS).build();
}
