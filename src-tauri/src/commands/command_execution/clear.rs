use crate::execution::ResultSourceStore;
use tauri::State;

/// Clear runtime pin result sources for one graph (manual "clear run artifacts").
/// Does not remove immutable `window_*` snapshots.
#[tauri::command]
pub fn clear_graph_execution_artifacts(
    source_store: State<'_, ResultSourceStore>,
    graph_path: String,
) -> Result<(), String> {
    let graph_path = crate::project::GraphResourcePath::new(graph_path).map_err(|e| e.to_string())?;
    source_store.clear_runtime_graph(graph_path.as_str());
    Ok(())
}
