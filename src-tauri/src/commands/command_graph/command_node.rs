use serde_json::Value;


#[tauri::command]
pub fn get_nodes() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn set_nodes(_nodes: Vec<Value>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn create_node(_node: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn create_nodes(_nodes: Vec<Value>) -> Result<Vec<String>, String> {
    Ok(vec![])
}

#[tauri::command]
pub fn create_nodes_with_connections(_data: Value) -> Result<Value, String> {
    Ok(Value::Null)
}

#[tauri::command]
pub fn delete_node(_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn connect_pins(_from: String, _to: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn disconnect_pin(_pin: String) -> Result<(), String> {
    Ok(())
}