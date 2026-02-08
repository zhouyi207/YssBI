use serde_json::Value;


#[tauri::command]
pub fn import_csv(_path: String) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn delete_dataframe(_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn create_dataframe(_data: Value) -> Result<String, String> {
    Ok("".to_string())
}

#[tauri::command]
pub fn get_dataframe_rows(_id: String, _start: usize, _count: usize) -> Result<Value, String> {
    Ok(Value::Null)
}