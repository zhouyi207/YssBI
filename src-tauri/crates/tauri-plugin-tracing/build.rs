const COMMANDS: &[&str] = &[
    "submit_frontend_logs",
    "subscribe_logs",
    "unsubscribe_logs",
    "query_logs",
    "log_statistics",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
