use serde_json::Value;


#[tauri::command]
pub fn get_macros() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn get_macro(_id: String) -> Option<Value> {
    None
}

#[tauri::command]
pub fn create_macro(_macro_data: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn update_macro(_id: String, _macro_data: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_macro(_id: String) -> Result<(), String> {
    Ok(())
}
