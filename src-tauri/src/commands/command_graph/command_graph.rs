use crate::event::emit_project_event;
use crate::event::{EventEvent, EventFunction};
use crate::graph::{GraphId, GraphKind};
use crate::log::log_app;
use crate::project::{read_project_index, GraphDocumentKind};
use crate::schema::GraphInstanceDTO;
use crate::{event::Event, project::ProjectState};
use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, State};

// #[tauri::command]
// pub fn execute_graph(_graph_id: String) -> Result<Value, String> {
//     Ok(Value::Null)
// }

fn existing_graph_names(
    state: &State<ProjectState>,
    graph_kind: GraphKind,
    excluded_id: Option<GraphId>,
) -> Result<Vec<String>, String> {
    let mut names: Vec<String> = state
        .project_data
        .read()
        .unwrap()
        .graphs
        .values()
        .filter(|graph| graph.kind == graph_kind && Some(graph.id) != excluded_id)
        .map(|graph| graph.name.clone())
        .collect();

    if let Some(path) = state.get_path() {
        let index = read_project_index(&path).map_err(|e| e.to_string())?;
        let expected_kind = GraphDocumentKind::from(&graph_kind);
        names.extend(
            index
                .graphs
                .into_iter()
                .filter(|graph| graph.graph_type == expected_kind && Some(graph.id) != excluded_id)
                .map(|graph| graph.name),
        );
    }

    names.sort();
    names.dedup();
    Ok(names)
}

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<String, String> {
    let graph = state.add_graph_with_existing_names(
        graph_name,
        GraphKind::Event,
        existing_graph_names(&state, GraphKind::Event, None)?,
    );
    let graph_id = graph.id.to_string();
    state.persist_current_project()?;
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
    let graph = state.add_graph_with_existing_names(
        graph_name,
        GraphKind::Function,
        existing_graph_names(&state, GraphKind::Function, None)?,
    );
    let graph_id = graph.id.to_string();
    state.persist_current_project()?;
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
    let loaded_graph = state.remove_graph(&graph_id);
    let removed_kind = if let Some(path) = state.get_path() {
        crate::project::remove_project_graph_from_file(&path, &graph_id)
            .map_err(|e| e.to_string())?
    } else {
        None
    };
    let graph_kind = loaded_graph
        .as_ref()
        .map(|graph| graph.kind.clone())
        .or_else(|| removed_kind.map(GraphKind::from))
        .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

    let event = match graph_kind {
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
    let update: GraphUpdateData =
        serde_json::from_value(data).map_err(|e| format!("Invalid update data: {}", e))?;

    if state.get_graph(&id).is_none() {
        state.load_graph_from_current_project(&id)?;
    }

    let next_name = if let Some(name) = update.name {
        let graph_kind = state
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(&id)
            .map(|graph| graph.kind.clone())
            .ok_or_else(|| format!("Graph '{}' not found", id))?;
        let existing_names = existing_graph_names(state, graph_kind, Some(id))?;
        Some(crate::project::unique_name::unique_name(
            &name,
            existing_names,
        ))
    } else {
        None
    };

    let mut project_data = state.project_data.write().unwrap();
    let graph = project_data
        .graphs
        .get_mut(&id)
        .ok_or_else(|| format!("Graph '{}' not found", id))?;

    if let Some(name) = next_name {
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
    state.persist_current_project()?;
    Ok(())
}

#[tauri::command]
pub fn update_event(
    app: AppHandle,
    state: State<ProjectState>,
    id: GraphId,
    event: Value,
) -> Result<(), String> {
    update_graph_inner(&app, &state, id, event)
}

#[tauri::command]
pub fn update_function(
    app: AppHandle,
    state: State<ProjectState>,
    id: GraphId,
    function: Value,
) -> Result<(), String> {
    update_graph_inner(&app, &state, id, function)
}

#[tauri::command]
pub fn get_graph(
    _app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
) -> Result<GraphInstanceDTO, String> {
    let graph = match state.get_graph(&graph_id) {
        Some(graph) => graph,
        None => state.load_graph_from_current_project(&graph_id)?.graph,
    };
    Ok((&graph).into())
}
