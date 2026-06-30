use crate::event::emit_project_event;
use crate::event::{EventEvent, EventFunction};
use crate::graph::{FunctionSignaturePin, GraphId, GraphKind};
use crate::project::{GraphDocumentKind, read_project_index};
use crate::schema::GraphInstanceDTO;
use crate::{event::Event, project::ProjectState};
use tauri::{AppHandle, State};

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

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    state: State<ProjectState>,
    function_id: GraphId,
    inputs: Option<Vec<FunctionSignaturePin>>,
    outputs: Option<Vec<FunctionSignaturePin>>,
) -> Result<GraphInstanceDTO, String> {
    let graph = state.update_function_signature(&function_id, inputs, outputs)?;
    let dto: GraphInstanceDTO = (&graph).into();
    emit_project_event(
        &app,
        Event::Function(EventFunction::FunctionUpdated {
            id: function_id,
            data: dto.clone(),
        }),
    );
    Ok(dto)
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

#[tauri::command]
pub fn unload_project_graph(state: State<ProjectState>, graph_id: GraphId) -> Result<(), String> {
    state.unload_graph(&graph_id);
    Ok(())
}

#[tauri::command]
pub fn save_project_graph(state: State<ProjectState>, graph_id: GraphId) -> Result<(), String> {
    state.persist_loaded_graph(&graph_id)
}

#[tauri::command]
pub fn duplicate_graph(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
) -> Result<GraphInstanceDTO, String> {
    let project_path = state.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
    let document = crate::project::duplicate_project_graph_file(&project_path, &graph_id)
        .map_err(|e| e.to_string())?;
    let graph = state.insert_loaded_graph(document.graph.clone(), document.local_variables.clone());
    let event = match graph.kind {
        GraphKind::Event => Event::Event(EventEvent::EventCreated {
            id: graph.id,
            data: (&graph).into(),
        }),
        GraphKind::Function => Event::Function(EventFunction::FunctionCreated {
            id: graph.id,
            data: (&graph).into(),
        }),
    };
    emit_project_event(&app, event);
    Ok((&graph).into())
}
