use crate::event::{Event, EventResource, ProjectResourceMetaEvent, emit_project_event};
use crate::graph::{GraphId, GraphKind};
use crate::project::{GraphDocumentKind, ProjectState, read_project_index};
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResourceMetaDTO {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uri: String,
    pub folder_path: Option<String>,
    pub exists: bool,
    pub loaded: bool,
    pub has_dirty_document: bool,
    pub has_stale_document: bool,
    pub has_conflict_document: bool,
}

impl From<&ProjectResourceMetaDTO> for ProjectResourceMetaEvent {
    fn from(value: &ProjectResourceMetaDTO) -> Self {
        Self {
            id: value.id.clone(),
            kind: value.kind.clone(),
            name: value.name.clone(),
            uri: value.uri.clone(),
            folder_path: value.folder_path.clone(),
            exists: value.exists,
            loaded: value.loaded,
            has_dirty_document: value.has_dirty_document,
            has_stale_document: value.has_stale_document,
            has_conflict_document: value.has_conflict_document,
        }
    }
}

fn graph_kind_to_resource_kind(kind: &GraphKind) -> &'static str {
    match kind {
        GraphKind::Event => "event",
        GraphKind::Function => "function",
    }
}

fn graph_uri(kind: &GraphKind, graph_id: &GraphId) -> String {
    format!(
        "yssbi://graph/{}/{}",
        graph_kind_to_resource_kind(kind),
        graph_id
    )
}

fn graph_folder_path(
    state: &ProjectState,
    graph_id: &GraphId,
    kind: &GraphKind,
) -> Result<Option<String>, String> {
    let Some(project_path) = state.get_path() else {
        return Ok(None);
    };
    let expected_kind = GraphDocumentKind::from(kind);
    let index = read_project_index(&project_path).map_err(|e| e.to_string())?;
    Ok(index
        .graphs
        .into_iter()
        .find(|entry| entry.id == *graph_id && entry.graph_type == expected_kind)
        .map(|entry| entry.folder_path))
}

fn graph_resource_meta(
    state: &ProjectState,
    graph_id: &GraphId,
    name: String,
    kind: GraphKind,
) -> Result<ProjectResourceMetaDTO, String> {
    Ok(ProjectResourceMetaDTO {
        id: graph_id.to_string(),
        kind: graph_kind_to_resource_kind(&kind).to_string(),
        name,
        uri: graph_uri(&kind, graph_id),
        folder_path: graph_folder_path(state, graph_id, &kind)?,
        exists: true,
        loaded: state.get_graph(graph_id).is_some(),
        has_dirty_document: false,
        has_stale_document: false,
        has_conflict_document: false,
    })
}

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    state: State<ProjectState>,
    graph_id: GraphId,
    new_name: String,
) -> Result<ProjectResourceMetaDTO, String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("Graph name cannot be empty".to_string());
    }

    if state.get_graph(&graph_id).is_none() {
        state.load_graph_from_current_project(&graph_id)?;
    }

    let (kind, final_name) = {
        let mut project_data = state.project_data.write().unwrap();
        let graph = project_data
            .graphs
            .get_mut(&graph_id)
            .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
        graph.name = trimmed.to_string();
        let kind = graph.kind.clone();
        let final_name = graph.name.clone();
        (kind, final_name)
    };

    state.persist_loaded_graph(&graph_id)?;

    let meta = graph_resource_meta(state.inner(), &graph_id, final_name, kind)?;
    emit_project_event(
        &app,
        Event::Resource(EventResource::ResourceChanged {
            id: meta.id.clone(),
            kind: meta.kind.clone(),
            source: "command".to_string(),
            data: (&meta).into(),
        }),
    );

    Ok(meta)
}
