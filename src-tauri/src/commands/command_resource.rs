use crate::event::{Event, EventResource, ProjectResourceMetaEvent, emit_project_event};
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

fn graph_uri(kind: &crate::graph::GraphKind, graph_path: &crate::project::GraphResourcePath) -> String {
    if crate::project::is_untitled_graph_path(graph_path.as_str()) {
        return graph_path.as_str().to_string();
    }
    crate::project::to_graph_resource_uri(
        crate::project::GraphDocumentKind::from(kind),
        graph_path,
    )
}

fn graph_resource_meta(
    state: &ProjectState,
    graph_path: &crate::project::GraphResourcePath,
    name: String,
    kind: crate::graph::GraphKind,
) -> Result<ProjectResourceMetaDTO, String> {
    let is_draft = crate::project::is_untitled_graph_path(graph_path.as_str());
    Ok(ProjectResourceMetaDTO {
        id: graph_path.as_str().to_string(),
        kind: graph_kind_to_resource_kind(&kind).to_string(),
        name,
        uri: graph_uri(&kind, graph_path),
        exists: !is_draft,
        loaded: state.get_graph(graph_path).is_some(),
        has_dirty_document: false,
        has_stale_document: false,
        has_conflict_document: false,
    })
}

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    state: State<ProjectState>,
    graph_path: String,
    new_name: String,
) -> Result<ProjectResourceMetaDTO, String> {
    let graph_path = crate::project::GraphResourcePath::new(graph_path).map_err(|e| e.to_string())?;
    let (final_name, kind, moved_to) = state.rename_graph(&graph_path, &new_name)?;

    let effective_path = moved_to.as_ref().unwrap_or(&graph_path);
    let meta = graph_resource_meta(state.inner(), effective_path, final_name, kind)?;
    emit_project_event(
        &app,
        Event::Resource(EventResource::ResourceChanged {
            id: meta.id.clone(),
            kind: meta.kind.clone(),
            source: "command".to_string(),
            data: (&meta).into(),
        }),
    );
    if let Some(to) = moved_to {
        emit_project_event(
            &app,
            Event::Resource(EventResource::GraphResourceMoved {
                from: graph_path.as_str().to_string(),
                to: to.as_str().to_string(),
                kind: meta.kind.clone(),
            }),
        );
    }

    Ok(meta)
}
