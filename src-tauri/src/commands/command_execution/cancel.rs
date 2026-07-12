use crate::project::ExecutionCancelRegistry;
use tauri::State;

#[tauri::command]
pub fn cancel_execution(cancel_registry: State<'_, ExecutionCancelRegistry>) {
    cancel_registry.cancel_active();
}
