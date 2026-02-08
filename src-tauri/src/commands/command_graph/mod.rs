pub mod command_connection;
pub mod command_event_crud;
pub mod command_function_crud;
pub mod command_macro_crud;
pub mod command_node;
pub mod command_pin;

pub use command_connection::*;
pub use command_event_crud::*;
pub use command_function_crud::*;
pub use command_macro_crud::*;
pub use command_node::*;
pub use command_pin::*;

use serde_json::Value;


#[tauri::command]
pub fn execute_graph(_graph_id: String) -> Result<Value, String> {
    Ok(Value::Null)
}