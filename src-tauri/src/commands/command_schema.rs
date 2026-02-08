use serde_json::Value;

#[tauri::command]
pub fn get_node_definitions() -> Vec<String> {
    vec![]
}

#[tauri::command]
pub fn get_editor_schema_command() -> Vec<String> {
    vec![]
}

#[tauri::command]
pub fn get_pin_types() -> Vec<String> {
    vec![]
}

#[tauri::command]
pub fn get_categories() -> Vec<String> {
    vec![]
}

#[tauri::command]
pub fn get_ui_styles() -> Vec<String> {
    vec![]
}

#[tauri::command]
pub fn get_variable_types() -> Vec<String> {
    vec![]
}

#[tauri::command]
pub fn check_type_connection(_from: String, _to: String) -> bool {
    true
}

#[tauri::command]
pub fn get_pin_type_info(_pin_type: String) -> Value {
    Value::Null
}

#[tauri::command]
pub fn check_pin_compatibility_detailed(_from: String, _to: String) -> Value {
    Value::Null
}
