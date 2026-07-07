use crate::execution::ResultSourceStore;
use crate::graph::GraphId;
use tauri::State;

/// Clear runtime pin result sources for one graph (manual "clear run artifacts").
/// Does not remove immutable `window_*` snapshots.
#[tauri::command]
pub fn clear_graph_execution_artifacts(
    source_store: State<'_, ResultSourceStore>,
    graph_id: String,
) -> Result<(), String> {
    let graph_id = uuid::Uuid::parse_str(&graph_id)
        .map(GraphId::from)
        .map_err(|e| format!("Invalid graph_id '{}': {}", graph_id, e))?;
    source_store.clear_runtime_graph(&graph_id.to_string());
    Ok(())
}
