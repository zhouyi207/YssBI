use crate::execution::ExecutionEvent;
use crate::graph::GraphId;
use crate::project::{ExecutionCancelRegistry, ProjectState, execute_project_data};
use serde_json::Value;
use tauri::{State, ipc::Channel};

/// 执行指定的 Event 图。
/// 若传入 graph_id 则只执行该图，否则执行所有 Event 图。
#[tauri::command]
pub async fn execute_project(
    state: State<'_, ProjectState>,
    source_store: State<'_, crate::execution::ResultSourceStore>,
    cancel_registry: State<'_, ExecutionCancelRegistry>,
    on_event: Channel<ExecutionEvent>,
    graph_id: Option<String>,
) -> Result<Value, String> {
    let source_store = source_store.inner().clone();
    let project_data_state = state.project_data.clone();
    let project_store = state.project_store.clone();
    let cancel = cancel_registry.begin();

    let target_graph_id: Option<GraphId> = graph_id
        .as_deref()
        .map(|s| {
            uuid::Uuid::parse_str(s)
                .map(GraphId::from)
                .map_err(|e| format!("Invalid graph_id '{}': {}", s, e))
        })
        .transpose()?;

    let cancel_for_task = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        execute_project_data(
            project_data_state,
            project_store,
            source_store,
            on_event,
            target_graph_id,
            cancel_for_task,
        )
    })
    .await
    .map_err(|e| e.to_string())?;

    cancel_registry.end(&cancel);
    result
}
