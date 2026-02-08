use serde_json::Value;


#[tauri::command]
pub fn get_global_variables() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn get_global_variable(_id: String) -> Option<Value> {
    None
}

#[tauri::command]
pub fn create_global_variable(_variable: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn update_global_variable(_id: String, _variable: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_global_variable(_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn get_local_variables() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn create_local_variable(_variable: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn create_variable(_variable: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn update_local_variable(_id: String, _variable: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_local_variable(_id: String) -> Result<(), String> {
    Ok(())
}