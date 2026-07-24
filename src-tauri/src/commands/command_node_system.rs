use crate::error::AppError;
use crate::event::{
    Event, EventProject, GraphProjectionReplacementDto, ResourceMutationResultDto,
    emit_project_event,
};
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::node_system::catalog::LocalizedCatalogDto;
use crate::node_system::document::{
    GraphDeltaEvent, GraphDocumentPatch, HistoryMutation, MutationRequest,
};
use crate::node_system::runtime::{
    ArtifactSnapshot, ResultSourceDescriptor, ResultSourceId, ResultSourcePage, RunEvent,
    RunEventSink,
};
use crate::project::{GraphResourcePath, ProjectState};
use serde::Serialize;
use tauri::{AppHandle, State, ipc::Channel};

fn parse_graph_path(value: String) -> Result<GraphResourcePath, AppError> {
    GraphResourcePath::new(value).map_err(AppError::from)
}

#[tauri::command]
pub fn get_localized_node_catalog(
    state: State<'_, ProjectState>,
    locale: String,
) -> Result<LocalizedCatalogDto, AppError> {
    let store = state.project_store.read().unwrap();
    Ok(store
        .catalog
        .localize(store.node_registry.as_ref(), &locale))
}

#[tauri::command]
pub fn create_event(
    state: State<'_, ProjectState>,
    graph_name: String,
) -> Result<String, AppError> {
    state
        .create_graph_resource(&graph_name, crate::project::GraphDocumentKind::Event)
        .map(|path| path.as_str().to_string())
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn create_function(
    state: State<'_, ProjectState>,
    graph_name: String,
) -> Result<String, AppError> {
    state
        .create_graph_resource(&graph_name, crate::project::GraphDocumentKind::Function)
        .map(|path| path.as_str().to_string())
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn unload_project_graph(
    state: State<'_, ProjectState>,
    graph_path: String,
) -> Result<(), AppError> {
    state.unload_graph_resource(&parse_graph_path(graph_path)?);
    Ok(())
}

#[tauri::command]
pub fn save_project_graph(
    state: State<'_, ProjectState>,
    graph_path: String,
) -> Result<SaveProjectGraphResult, AppError> {
    let graph_path = parse_graph_path(graph_path)?;
    state
        .save_graph_resource(&graph_path)
        .map(|path| SaveProjectGraphResult { path })
        .map_err(AppError::internal)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProjectGraphResult {
    pub path: String,
}

#[tauri::command]
pub fn duplicate_graph(
    state: State<'_, ProjectState>,
    graph_path: String,
) -> Result<String, AppError> {
    state
        .duplicate_graph_resource(&parse_graph_path(graph_path)?)
        .map(|path| path.as_str().to_string())
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
) -> Result<(), AppError> {
    state
        .remove_graph_resource(&parse_graph_path(graph_path)?)
        .map_err(AppError::internal)?;
    crate::event::emit_project_index_invalidated(&app, "remove_graph");
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResourceMetaDto {
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

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
    new_name: String,
) -> Result<GraphResourceMetaDto, AppError> {
    let path = state
        .rename_graph_resource(&parse_graph_path(graph_path)?, &new_name)
        .map_err(AppError::internal)?;
    let kind = path.kind().map_err(AppError::from)?;
    crate::event::emit_project_index_invalidated(&app, "rename_graph_resource");
    Ok(GraphResourceMetaDto {
        id: path.as_str().to_string(),
        kind: match kind {
            crate::project::GraphDocumentKind::Event => "event",
            crate::project::GraphDocumentKind::Function => "function",
        }
        .into(),
        name: new_name,
        uri: path.as_str().to_string(),
        exists: true,
        loaded: false,
        has_dirty_document: false,
        has_stale_document: false,
        has_conflict_document: false,
    })
}

#[tauri::command]
pub fn update_function_signature(
    app: AppHandle,
    state: State<'_, ProjectState>,
    function_path: String,
    locale: String,
    request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
) -> Result<ResourceMutationResultDto, AppError> {
    let path = parse_graph_path(function_path)?;
    let delta = state
        .update_function_signature(&path, request)
        .map_err(|error| match error {
            crate::node_system::document::MutationConflict::StaleRevision { .. } => {
                AppError::new("function_revision_conflict", error.to_string())
            }
            _ => AppError::internal(error),
        })?;
    let result = resource_mutation_result(state.inner(), &locale, vec![delta])?;
    emit_project_event(
        &app,
        Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    );
    Ok(result)
}

#[tauri::command]
pub fn hydrate_editor_graph(
    state: State<'_, ProjectState>,
    graph_path: String,
    locale: String,
) -> Result<EditorGraphProjectionDto, AppError> {
    state
        .graph_projection(&parse_graph_path(graph_path)?, &locale)
        .map_err(AppError::internal)
}

#[tauri::command]
pub fn mutate_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
    request: MutationRequest<GraphDocumentPatch>,
) -> Result<GraphDeltaEvent<GraphDocumentPatch>, AppError> {
    let delta = state
        .apply_graph_patch(&parse_graph_path(graph_path)?, request)
        .map_err(|error| match error {
            crate::node_system::document::MutationConflict::StaleRevision { .. } => {
                AppError::new("graph_revision_conflict", error.to_string())
            }
            _ => AppError::internal(error),
        })?;
    emit_project_event(
        &app,
        Event::Project(EventProject::GraphDelta {
            delta: delta.clone(),
        }),
    );
    Ok(delta)
}

fn resource_mutation_result(
    state: &ProjectState,
    locale: &str,
    deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
) -> Result<ResourceMutationResultDto, AppError> {
    let graph_paths = deltas
        .iter()
        .filter_map(|delta| match &delta.resource {
            crate::node_system::document::ResourceKey::Graph(path) => Some(path.0.to_string()),
            crate::node_system::document::ResourceKey::Function(path) => Some(path.0.to_string()),
            crate::node_system::document::ResourceKey::Variable(_) => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let projection_replacements = graph_paths
        .into_iter()
        .map(|graph_path| {
            let path = GraphResourcePath::new(&graph_path).map_err(AppError::from)?;
            let projection = state
                .graph_projection(&path, locale)
                .map_err(AppError::internal)?;
            Ok(GraphProjectionReplacementDto {
                graph_path,
                projection,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(ResourceMutationResultDto {
        deltas,
        projection_replacements,
    })
}

fn emit_resource_mutation_result(app: &AppHandle, result: &ResourceMutationResultDto) {
    emit_project_event(
        app,
        Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    );
}

#[tauri::command]
pub fn undo_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, AppError> {
    let deltas = state
        .undo_last_transaction(request)
        .map_err(|error| match error {
            crate::node_system::document::MutationConflict::StaleRevision { .. } => {
                AppError::new("history_revision_conflict", error.to_string())
            }
            _ => AppError::internal(error),
        })?;
    let result = resource_mutation_result(state.inner(), &locale, deltas)?;
    emit_resource_mutation_result(&app, &result);
    Ok(result)
}

#[tauri::command]
pub fn redo_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, AppError> {
    let deltas = state
        .redo_last_transaction(request)
        .map_err(|error| match error {
            crate::node_system::document::MutationConflict::StaleRevision { .. } => {
                AppError::new("history_revision_conflict", error.to_string())
            }
            _ => AppError::internal(error),
        })?;
    let result = resource_mutation_result(state.inner(), &locale, deltas)?;
    emit_resource_mutation_result(&app, &result);
    Ok(result)
}

struct ChannelRunEvents(Channel<RunEvent>);

impl RunEventSink for ChannelRunEvents {
    fn record(&self, event: RunEvent) {
        let _ = self.0.send(event);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteGraphResultDto {
    pub run_id: u64,
}

#[tauri::command]
pub fn get_result_source_descriptor(
    state: State<'_, ProjectState>,
    source_id: u64,
) -> Option<ResultSourceDescriptor> {
    state
        .project_store
        .read()
        .unwrap()
        .results
        .descriptor(ResultSourceId::new(source_id))
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ResultSourceValueDto {
    Value(crate::node_system::protocol::Value),
    Sequence(Box<[crate::node_system::protocol::Value]>),
}

#[tauri::command]
pub fn get_result_source_value(
    state: State<'_, ProjectState>,
    source_id: u64,
) -> Option<ResultSourceValueDto> {
    state
        .project_store
        .read()
        .unwrap()
        .results
        .value(ResultSourceId::new(source_id))
        .map(|snapshot| match snapshot.as_ref() {
            ArtifactSnapshot::Value(value) => ResultSourceValueDto::Value(value.clone()),
            ArtifactSnapshot::Sequence(values) => ResultSourceValueDto::Sequence(values.clone()),
        })
}

#[tauri::command]
pub fn get_result_source_page(
    state: State<'_, ProjectState>,
    source_id: u64,
    offset: usize,
    limit: usize,
) -> Option<ResultSourcePage> {
    state
        .project_store
        .read()
        .unwrap()
        .results
        .page(ResultSourceId::new(source_id), offset, limit)
}

#[tauri::command]
pub fn release_result_source(state: State<'_, ProjectState>, source_id: u64) -> bool {
    state
        .project_store
        .read()
        .unwrap()
        .results
        .release(ResultSourceId::new(source_id))
}

#[tauri::command]
pub fn release_run_result_sources(state: State<'_, ProjectState>, run_id: u64) -> usize {
    state
        .project_store
        .read()
        .unwrap()
        .results
        .release_run_sources(crate::node_system::analysis::RunId::new(run_id))
}

#[tauri::command]
pub async fn execute_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
    on_event: Channel<RunEvent>,
) -> Result<ExecuteGraphResultDto, AppError> {
    let graph_path = parse_graph_path(graph_path)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .execute_graph(&graph_path, &ChannelRunEvents(on_event))
            .map(|result| {
                if !result.resource_deltas.is_empty() {
                    emit_resource_mutation_result(
                        &app,
                        &ResourceMutationResultDto {
                            deltas: result.resource_deltas.clone(),
                            projection_replacements: Vec::new(),
                        },
                    );
                }
                for variable_id in &result.committed_variable_ids {
                    if let Some(variable) = state.get_variable(variable_id) {
                        emit_project_event(
                            &app,
                            Event::Variable(crate::event::EventVariable::VariableUpdated {
                                variable_id: *variable_id,
                                variable_scope: variable.scope.clone(),
                                data: (&variable).into(),
                            }),
                        );
                    }
                }
                ExecuteGraphResultDto {
                    run_id: result.run_id.get(),
                }
            })
            .map_err(AppError::internal)
    })
    .await
    .map_err(AppError::internal)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::document::{
        GraphDocumentPatch, GraphResourcePath as DocumentGraphResourcePath, OperationId,
        ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey, ResourceRevision,
    };
    use crate::project::{GraphDocumentKind, GraphResourceDocument};

    #[test]
    fn resource_mutation_result_replaces_committed_graph_projection() {
        let state = ProjectState::new();
        let path = GraphResourcePath::new("events/Replacement.yssbi-event").unwrap();
        state.insert_graph(
            path.clone(),
            GraphResourceDocument::new("Replacement", GraphDocumentKind::Event),
        );
        let operation_id = OperationId::new();
        let patch = GraphDocumentPatch::new(Vec::new());
        state
            .apply_graph_patch(
                &path,
                MutationRequest::new(
                    ResourceKey::Graph(DocumentGraphResourcePath(path.as_str().into())),
                    ResourceRevision::INITIAL,
                    operation_id,
                    patch.clone(),
                ),
            )
            .unwrap();
        let result = resource_mutation_result(
            &state,
            "en-US",
            vec![ResourceDeltaEvent {
                resource: ResourceKey::Graph(DocumentGraphResourcePath(path.as_str().into())),
                from_revision: ResourceRevision::INITIAL,
                to_revision: ResourceRevision::new(1),
                caused_by: Some(operation_id),
                payload: ResourceDocumentPatch::Graph(patch),
            }],
        )
        .unwrap();

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.projection_replacements.len(), 1);
        assert_eq!(result.projection_replacements[0].graph_path, path.as_str());
        assert_eq!(
            result.projection_replacements[0].projection.source_revision,
            1
        );
    }
}
