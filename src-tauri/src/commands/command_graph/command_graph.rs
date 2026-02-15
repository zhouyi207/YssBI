use crate::event::emit_project_event;
use crate::event::{EventEvent, EventFunction, EventMacro};
use crate::graph::{GraphId, GraphKind};
use crate::schema::GraphInstanceDTO;
use crate::{event::Event, project::ProjectState};
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
pub fn create_macro(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<String, String> {
    let graph = state.add_macro(graph_name);
    let graph_id = graph.id.to_string();
    emit_project_event(
        &app,
        Event::Macro(EventMacro::MacroCreated {
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
        GraphKind::Macro => Event::Macro(EventMacro::MacroDeleted { id: graph_id }),
    };
    emit_project_event(&app, event);
    Ok(())
}

#[tauri::command]
pub fn update_event(_id: String, _event: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn update_function(_id: String, _function: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn update_macro(_id: String, _macro_data: Value) -> Result<(), String> {
    Ok(())
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
