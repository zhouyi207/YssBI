use crate::graph::{GraphId, NodeId};
use crate::project::ProjectState;
use tauri::State;

#[tauri::command]
pub fn create_node(
    state: State<ProjectState>,
    graph_id: GraphId,
    node_type: &str,
) -> Result<(), String> {
    let bounding = state.project_data.write().unwrap();
    let graph = bounding.graphs.get(&graph_id);
    let _ = graph.unwrap().create_node(node_type);
    Ok(())
}

#[tauri::command]
pub fn delete_node(
    state: State<ProjectState>,
    graph_id: GraphId,
    node_id: NodeId,
) -> Result<(), String> {
    let bounding = state.project_data.write().unwrap();
    let graph = bounding.graphs.get(&graph_id);
    let _ = graph.unwrap().remove_node(node_id);
    Ok(())
}
