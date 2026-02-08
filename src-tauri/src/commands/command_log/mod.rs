use serde_json::Value;


#[tauri::command]
pub fn get_logs() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn get_log_file_path() -> String {
    "".to_string()
}

#[tauri::command]
pub fn get_log_count() -> usize {
    0
}
