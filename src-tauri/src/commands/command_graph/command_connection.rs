use serde_json::Value;


#[tauri::command]
pub fn create_connection(_from: String, _to: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_connection(_from: String, _to: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn get_connections() -> Vec<Value> {
    vec![]
}

#[tauri::command]
pub fn delete_connections_for_pin(_pin: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn delete_connections_for_node(_node: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn update_canvas(_data: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn update_subgraph_io(_id: String, _data: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn rename_subgraph(_id: String, _name: String) -> Result<(), String> {
    Ok(())
}