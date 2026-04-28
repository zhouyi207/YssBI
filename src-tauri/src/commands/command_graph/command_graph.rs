use crate::event::emit_project_event;
use crate::event::{EventEvent, EventFunction};
use crate::graph::{GraphId, GraphKind};
use crate::log::log_app;
use crate::schema::GraphInstanceDTO;
use crate::{event::Event, project::ProjectState};
use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, State};

// #[tauri::command]
// pub fn execute_graph(_graph_id: String) -> Result<Value, String> {
//     Ok(Value::Null)
// }

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<String, String> {
    let graph = state.add_event(graph_name);
    let graph_id = graph.id.to_string();
    emit_project_event(
        &app,
        Event::Event(EventEvent::EventCreated {
            id: graph.id,
            data: (&graph).into(),
        }),
    );
    Ok(graph_id)
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<String, String> {
    let graph = state.add_function(graph_name);
    let graph_id = graph.id.to_string();
    emit_project_event(
        &app,
        Event::Function(EventFunction::FunctionCreated {
            id: graph.id,
            data: (&graph).into(),
        }),
    );
    Ok(graph_id)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
) -> Result<(), String> {
    let graph = state.remove_graph(&graph_id).unwrap();
    let event = match graph.kind {
        GraphKind::Event => Event::Event(EventEvent::EventDeleted { id: graph_id }),
        GraphKind::Function => Event::Function(EventFunction::FunctionDeleted { id: graph_id }),
    };
    emit_project_event(&app, event);
    Ok(())
}

/// 从 Value 中提取可更新的图属性
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphUpdateData {
    name: Option<String>,
}

/// 通用的图更新逻辑
fn update_graph_inner(
    app: &AppHandle,
    state: &State<ProjectState>,
    id: GraphId,
    data: Value,
) -> Result<(), String> {
    let update: GraphUpdateData = serde_json::from_value(data)
        .map_err(|e| format!("Invalid update data: {}", e))?;

    let mut project_data = state.project_data.write().unwrap();
    let graph = project_data
        .graphs
        .get_mut(&id)
        .ok_or_else(|| format!("Graph '{}' not found", id))?;

    if let Some(name) = update.name {
        log_app::info!("[command.update_graph] graph={}, new_name={}", id, name);
        graph.name = name;
    }

    let dto: GraphInstanceDTO = (&*graph).into();
    let event = match graph.kind {
        GraphKind::Event => Event::Event(EventEvent::EventUpdated { id, data: dto }),
        GraphKind::Function => Event::Function(EventFunction::FunctionUpdated { id, data: dto }),
    };
    drop(project_data);
    emit_project_event(app, event);
    Ok(())
}

#[tauri::command]
pub fn update_event(app: AppHandle, state: State<ProjectState>, id: GraphId, event: Value) -> Result<(), String> {
    update_graph_inner(&app, &state, id, event)
}

#[tauri::command]
pub fn update_function(app: AppHandle, state: State<ProjectState>, id: GraphId, function: Value) -> Result<(), String> {
    update_graph_inner(&app, &state, id, function)
}

#[tauri::command]
pub fn get_graph(
    _app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
) -> Result<GraphInstanceDTO, String> {
    let graph = state.get_graph(&graph_id).unwrap();
    Ok((&graph).into())
}
