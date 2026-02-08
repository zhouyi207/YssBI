use serde_json::Value;


#[tauri::command]
pub fn get_events() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn get_event(_id: String) -> Option<Value> {
    None
}

#[tauri::command]
pub fn create_event(_event: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn update_event(_id: String, _event: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_event(_id: String) -> Result<(), String> {
    Ok(())
}