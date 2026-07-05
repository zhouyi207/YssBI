use crate::event::{Event, EventResource, ProjectResourceMetaEvent, emit_project_event};
use crate::graph::GraphId;
use crate::project::ProjectState;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResourceMetaDTO {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uri: String,
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
            exists: value.exists,
            loaded: value.loaded,
            has_dirty_document: value.has_dirty_document,
            has_stale_document: value.has_stale_document,
            has_conflict_document: value.has_conflict_document,
        }
    }
}

fn graph_kind_to_resource_kind(kind: &crate::graph::GraphKind) -> &'static str {
    match kind {
        crate::graph::GraphKind::Event => "event",
        crate::graph::GraphKind::Function => "function",
    }
}

fn graph_uri(kind: &crate::graph::GraphKind, graph_id: &GraphId) -> String {
    format!(
        "yssbi://graph/{}/{}",
        graph_kind_to_resource_kind(kind),
        graph_id
    )
}

fn graph_resource_meta(
    state: &ProjectState,
    graph_id: &GraphId,
    name: String,
    kind: crate::graph::GraphKind,
) -> Result<ProjectResourceMetaDTO, String> {
    Ok(ProjectResourceMetaDTO {
        id: graph_id.to_string(),
        kind: graph_kind_to_resource_kind(&kind).to_string(),
        name,
        uri: graph_uri(&kind, graph_id),
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
    let (final_name, kind) = state.rename_graph(&graph_id, &new_name)?;

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
