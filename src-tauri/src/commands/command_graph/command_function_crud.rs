use serde_json::Value;


#[tauri::command]
pub fn get_functions() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn get_function(_id: String) -> Option<Value> {
    None
}

#[tauri::command]
pub fn create_function(_function: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn update_function(_id: String, _function: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_function(_id: String) -> Result<(), String> {
    Ok(())
}