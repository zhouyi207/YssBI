use crate::log::log_app;
use crate::schema::NodeDefinitionDTO;
use crate::{
    project::ProjectState,
    schema::{get_editor_schema, EditorSchema},
};
use tauri::State;

// 获取所有节点定义
#[tauri::command]
pub fn get_node_definitions(state: State<ProjectState>) -> Vec<NodeDefinitionDTO> {
    log_app::info!("get_node_definitions command called");

    let node_register = &state.project_store.read().unwrap().node_register;
    let all_nodes = node_register.all();

    log_app::debug!("Node registry has {} nodes", all_nodes.len());

    let result: Vec<NodeDefinitionDTO> = all_nodes
        .iter()
        .map(|def| NodeDefinitionDTO::from(def.as_ref()))
        .collect();

    log_app::debug!("Returning {} node definitions to frontend", result.len());

    result
}

/// 获取完整的编辑器 Schema（一次性获取所有元数据）
#[tauri::command]
pub fn get_editor_schema_command() -> EditorSchema {
    get_editor_schema()
}
