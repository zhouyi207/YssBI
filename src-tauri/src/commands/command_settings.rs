use serde_json::Value;


#[tauri::command]
pub fn load_settings() -> Value {
    Value::Null
}

#[tauri::command]
pub fn save_settings(_settings: Value) -> Result<(), String> {
    Ok(())
}