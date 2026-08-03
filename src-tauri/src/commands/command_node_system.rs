use crate::commands::node_system_execution_dto::{
    ExecutionDemandDto, ResultSourceDescriptorDto, ResultSourcePageDto, RunEventDto,
};
use crate::error::AppError;
use crate::event::{
    Event, EventProject, GraphMutationResultDto, ResourceMutationResultDto, emit_project_event,
};
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::node_system::catalog::LocalizedCatalogDto;
use crate::node_system::document::{
    EditorGraphMutationDto, HistoryMutation, HistoryStatusDto, MutationRequest, OperationId,
    ResourceRevision,
};
use crate::node_system::runtime::{ArtifactSnapshot, ResultSourceId, RunEvent, RunEventSink};
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{GraphResourcePath, ProjectFilesystemError, ProjectInstanceId, ProjectState};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::{AppHandle, State, ipc::Channel};

fn parse_graph_path(value: String) -> Result<GraphResourcePath, AppError> {
    GraphResourcePath::new(value).map_err(AppError::from)
}

fn parse_opaque_u64(field: &'static str, value: &str) -> Result<u64, AppError> {
    value.parse::<u64>().map_err(|_| AppError {
        code: "invalid_opaque_id".into(),
        message: format!("'{value}' is not a valid decimal {field}"),
        details: Some(serde_json::json!({ "field": field })),
    })
}

fn mutation_conflict_to_app_error(
    error: crate::node_system::document::MutationConflict,
    revision_conflict_code: &'static str,
) -> AppError {
    match error {
        crate::node_system::document::MutationConflict::RecoveryRequired(message) => AppError {
            code: "project_recovery_required".into(),
            message: message.into(),
            details: Some(serde_json::json!({ "recoveryRequired": true })),
        },
        catalog_error
        @ (crate::node_system::document::MutationConflict::CatalogResourceStale(_)
        | crate::node_system::document::MutationConflict::CatalogDescriptorInvalid(_)) => {
            AppError::new(catalog_error.code(), catalog_error.to_string())
        }
        crate::node_system::document::MutationConflict::StaleRevision { .. } => {
            AppError::new(revision_conflict_code, error.to_string())
        }
        _ => AppError::internal(error),
    }
}

fn get_localized_node_catalog_from_state(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    locale: &str,
) -> Result<LocalizedCatalogDto, AppError> {
    state
        .localized_catalog_snapshot(&project_instance_id, locale)
        .map_err(|error| match error {
            ProjectFilesystemError::StaleProjectLifecycle { .. } => {
                AppError::new("catalog_project_stale", error.to_string())
            }
            _ => AppError::from(error),
        })
}

#[tauri::command]
pub fn get_localized_node_catalog(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    locale: String,
) -> Result<LocalizedCatalogDto, AppError> {
    get_localized_node_catalog_from_state(state.inner(), project_instance_id, &locale)
}

trait EmitOutcome {
    fn discard(self);
}

impl EmitOutcome for () {
    fn discard(self) {}
}

impl<E> EmitOutcome for Result<(), E> {
    fn discard(self) {}
}

fn emit_resource_result<R: EmitOutcome>(
    emit: &mut impl FnMut(Event) -> R,
    result: &ResourceMutationResultDto,
) {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.clone(),
    }))
    .discard();
}

fn create_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_name: &str,
    kind: crate::project::GraphDocumentKind,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, AppError> {
    let result = state.create_graph_resource_transaction(
        &project_instance_id,
        graph_name,
        kind,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn create_event(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, AppError> {
    create_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        &graph_name,
        crate::project::GraphDocumentKind::Event,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn create_function(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, AppError> {
    create_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        &graph_name,
        crate::project::GraphDocumentKind::Function,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn unload_project_graph(
    state: State<'_, ProjectState>,
    project_instance_id: String,
    graph_path: String,
    lifecycle_token: u64,
) -> Result<(), AppError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    state.unload_graph_resource_for_lifecycle(
        &project_instance_id,
        &parse_graph_path(graph_path)?,
        lifecycle_token,
    )?;
    Ok(())
}

fn save_project_graph_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<ProjectSaveResultDto, AppError> {
    let result = state
        .save_graph_document(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )
        .map_err(AppError::from)?;
    emit(Event::Project(EventProject::ProjectSaved {
        result: result.clone(),
    }));
    Ok(result)
}

#[tauri::command]
pub fn save_project_graph(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ProjectSaveResultDto, AppError> {
    save_project_graph_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

fn duplicate_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, AppError> {
    let result = state.duplicate_graph_resource_transaction(
        &project_instance_id,
        &graph_path,
        expected_revision,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn duplicate_graph(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, AppError> {
    duplicate_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

fn remove_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, AppError> {
    let result = state.remove_graph_resource_transaction(
        &project_instance_id,
        &graph_path,
        expected_revision,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn remove_graph(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, AppError> {
    remove_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

fn rename_graph_resource_with_emitter<R: EmitOutcome>(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: ResourceRevision,
    new_name: &str,
    lifecycle_token: u64,
    operation_id: OperationId,
    mut emit: impl FnMut(Event) -> R,
) -> Result<ResourceMutationResultDto, AppError> {
    let result = state.rename_graph_resource_transaction(
        &project_instance_id,
        &graph_path,
        expected_revision,
        new_name,
        lifecycle_token,
        operation_id,
    )?;
    emit_resource_result(&mut emit, &result);
    Ok(result)
}

#[tauri::command]
pub fn rename_graph_resource(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, AppError> {
    rename_graph_resource_with_emitter(
        state.inner(),
        project_instance_id,
        parse_graph_path(graph_path)?,
        expected_revision,
        &new_name,
        lifecycle_token,
        operation_id,
        |event| emit_project_event(&app, event),
    )
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
    state
        .update_function_signature_observed(&path, &locale, request, |result| {
            emit_resource_mutation_result(&app, result)
        })
        .map_err(|error| mutation_conflict_to_app_error(error, "function_revision_conflict"))
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

fn parse_editor_mutation_request(
    request: serde_json::Value,
) -> Result<MutationRequest<EditorGraphMutationDto>, AppError> {
    serde_json::from_value(request.clone()).map_err(|error| {
        let code = if is_create_node_descriptor_shape_error(&request) {
            "catalog_descriptor_invalid"
        } else {
            "invalid_editor_mutation"
        };
        AppError::new(code, format!("invalid editor mutation request: {error}"))
    })
}

fn is_create_node_descriptor_shape_error(request: &serde_json::Value) -> bool {
    let Some(mutation) = request
        .get("payload")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    if mutation.get("type").and_then(serde_json::Value::as_str) != Some("createNode") {
        return false;
    }
    let Some(create) = mutation
        .get("payload")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    if create.contains_key("parameters") {
        return true;
    }
    create.get("descriptor").is_none_or(|descriptor| {
        serde_json::from_value::<crate::node_system::catalog::NodeCreationDescriptor>(
            descriptor.clone(),
        )
        .is_err()
    })
}

fn mutate_graph_document_with_emitter(
    state: &ProjectState,
    graph_path: String,
    locale: &str,
    request: serde_json::Value,
    mut emit: impl FnMut(Event),
) -> Result<GraphMutationResultDto, AppError> {
    let request = parse_editor_mutation_request(request)?;
    state
        .apply_editor_graph_mutation_observed(
            &parse_graph_path(graph_path)?,
            locale,
            request,
            |delta| {
                emit(Event::Project(EventProject::GraphDelta {
                    delta: delta.clone(),
                }))
            },
        )
        .map_err(|error| mutation_conflict_to_app_error(error, "graph_revision_conflict"))
}

#[tauri::command]
pub fn mutate_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
    locale: String,
    request: serde_json::Value,
) -> Result<GraphMutationResultDto, AppError> {
    mutate_graph_document_with_emitter(state.inner(), graph_path, &locale, request, |event| {
        emit_project_event(&app, event)
    })
}

#[tauri::command]
pub fn get_project_history_status(
    state: State<'_, ProjectState>,
) -> Result<HistoryStatusDto, AppError> {
    state.ensure_project_operational().map_err(AppError::from)?;
    Ok(state.history_status())
}

fn emit_resource_mutation_result(app: &AppHandle, result: &ResourceMutationResultDto) {
    emit_project_event(
        app,
        Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    );
}

fn publish_run_resource_mutation(
    resource_mutation: Option<&ResourceMutationResultDto>,
    mut emit: impl FnMut(Event),
) {
    if let Some(result) = resource_mutation {
        emit(Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }));
    }
}

#[tauri::command]
pub fn undo_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, AppError> {
    state
        .undo_last_transaction_observed(&locale, request, |result| {
            emit_resource_mutation_result(&app, result)
        })
        .map_err(|error| mutation_conflict_to_app_error(error, "history_revision_conflict"))
}

#[tauri::command]
pub fn redo_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    locale: String,
    request: MutationRequest<HistoryMutation>,
) -> Result<ResourceMutationResultDto, AppError> {
    state
        .redo_last_transaction_observed(&locale, request, |result| {
            emit_resource_mutation_result(&app, result)
        })
        .map_err(|error| mutation_conflict_to_app_error(error, "history_revision_conflict"))
}

#[derive(Clone, Copy)]
enum TerminalRunEvent {
    Errored = 1,
    Cancelled = 2,
}

impl TerminalRunEvent {
    fn from_state(state: u8) -> Option<Self> {
        match state {
            1 => Some(Self::Errored),
            2 => Some(Self::Cancelled),
            _ => None,
        }
    }
}

struct ChannelRunEvents {
    channel: Channel<RunEventDto>,
    terminal: Arc<AtomicU8>,
}

impl RunEventSink for ChannelRunEvents {
    fn record(&self, event: RunEvent) {
        let terminal = match &event.kind {
            crate::node_system::runtime::RunEventKind::RunErrored { .. } => {
                Some(TerminalRunEvent::Errored)
            }
            crate::node_system::runtime::RunEventKind::RunCancelled => {
                Some(TerminalRunEvent::Cancelled)
            }
            _ => None,
        };
        if self.channel.send(event.into()).is_ok() {
            if let Some(terminal) = terminal {
                self.terminal.store(terminal as u8, Ordering::Release);
            }
        }
    }
}

fn execution_app_error(message: String, terminal: Option<TerminalRunEvent>) -> AppError {
    let Some(terminal) = terminal else {
        if message.starts_with("invalid_execution_demand:") {
            return AppError {
                code: "invalid_execution_demand".into(),
                message,
                details: None,
            };
        }
        return AppError::internal(message);
    };
    AppError {
        code: match terminal {
            TerminalRunEvent::Errored => "run_failed",
            TerminalRunEvent::Cancelled => "run_cancelled",
        }
        .into(),
        message,
        details: Some(serde_json::json!({ "terminalRunEventSent": true })),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteGraphResultDto {
    pub run_id: String,
}

#[tauri::command]
pub fn get_result_source_descriptor(
    state: State<'_, ProjectState>,
    source_id: String,
) -> Result<Option<ResultSourceDescriptorDto>, AppError> {
    let source_id = parse_opaque_u64("sourceId", &source_id)?;
    state
        .result_source_descriptor(ResultSourceId::new(source_id))
        .map_err(AppError::from)
        .map(|descriptor| descriptor.map(Into::into))
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
    source_id: String,
) -> Result<Option<ResultSourceValueDto>, AppError> {
    let source_id = parse_opaque_u64("sourceId", &source_id)?;
    state
        .result_source_value(ResultSourceId::new(source_id))
        .map_err(AppError::from)
        .map(|snapshot| {
            snapshot.map(|snapshot| match snapshot.as_ref() {
                ArtifactSnapshot::Value(value) => ResultSourceValueDto::Value(value.clone()),
                ArtifactSnapshot::Sequence(values) => {
                    ResultSourceValueDto::Sequence(values.clone())
                }
            })
        })
}

#[tauri::command]
pub fn get_result_source_page(
    state: State<'_, ProjectState>,
    source_id: String,
    offset: usize,
    limit: usize,
) -> Result<Option<ResultSourcePageDto>, AppError> {
    let source_id = parse_opaque_u64("sourceId", &source_id)?;
    state
        .result_source_page(ResultSourceId::new(source_id), offset, limit)
        .map_err(AppError::from)
        .map(|page| page.map(Into::into))
}

#[tauri::command]
pub fn release_result_source(
    state: State<'_, ProjectState>,
    source_id: String,
) -> Result<bool, AppError> {
    let source_id = parse_opaque_u64("sourceId", &source_id)?;
    state
        .release_result_source(ResultSourceId::new(source_id))
        .map_err(AppError::from)
}

#[tauri::command]
pub fn release_run_result_sources(
    state: State<'_, ProjectState>,
    run_id: String,
) -> Result<usize, AppError> {
    let run_id = parse_opaque_u64("runId", &run_id)?;
    state
        .release_run_result_sources(crate::node_system::analysis::RunId::new(run_id))
        .map_err(AppError::from)
}

#[tauri::command]
pub fn cancel_graph_run(state: State<'_, ProjectState>, run_id: String) -> Result<bool, AppError> {
    let run_id = parse_opaque_u64("runId", &run_id)?;
    Ok(state.cancel_graph_run(crate::node_system::analysis::RunId::new(run_id)))
}

#[tauri::command]
pub async fn execute_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
    demand: ExecutionDemandDto,
    on_event: Channel<RunEventDto>,
) -> Result<ExecuteGraphResultDto, AppError> {
    let graph_path = parse_graph_path(graph_path)?;
    let demand =
        crate::node_system::plan::ExecutionDemand::try_from(demand).map_err(|message| {
            AppError {
                code: "invalid_execution_demand".into(),
                message,
                details: None,
            }
        })?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let terminal = Arc::new(AtomicU8::new(0));
        let events = ChannelRunEvents {
            channel: on_event,
            terminal: Arc::clone(&terminal),
        };
        state
            .execute_graph(&graph_path, &demand, &events)
            .map(|result| {
                publish_run_resource_mutation(result.resource_mutation.as_ref(), |event| {
                    emit_project_event(&app, event)
                });
                ExecuteGraphResultDto {
                    run_id: result.run_id.get().to_string(),
                }
            })
            .map_err(|error| {
                execution_app_error(
                    error,
                    TerminalRunEvent::from_state(terminal.load(Ordering::Acquire)),
                )
            })
    })
    .await
    .map_err(AppError::internal)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, CorrelationContext, ParentCallId, ProjectSessionId, RunId,
    };
    use crate::node_system::catalog::NodeCreationDescriptor;
    use crate::node_system::document::{
        ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey, ResourceRevision,
        VariableDocumentPatch, VariableResourceKey,
    };
    use crate::node_system::registry::RegistryFingerprint;
    use crate::node_system::runtime::{ResultSourceId, RunEventKind};
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData, fixtures,
    };

    #[test]
    fn execution_errors_report_terminal_delivery_and_stable_codes() {
        let cancelled = execution_app_error(
            "run was cancelled".into(),
            Some(TerminalRunEvent::Cancelled),
        );
        let failed =
            execution_app_error("operation failed".into(), Some(TerminalRunEvent::Errored));
        let pre_run = execution_app_error("compile failed".into(), None);
        let invalid_demand = execution_app_error(
            "invalid_execution_demand: requested output node is missing".into(),
            None,
        );

        assert_eq!(cancelled.code, "run_cancelled");
        assert_eq!(failed.code, "run_failed");
        assert_eq!(pre_run.code, "internal_error");
        assert_eq!(invalid_demand.code, "invalid_execution_demand");
        assert_eq!(
            cancelled.details,
            Some(serde_json::json!({ "terminalRunEventSent": true })),
        );
        assert_eq!(
            failed.details,
            Some(serde_json::json!({ "terminalRunEventSent": true })),
        );
        assert!(pre_run.details.is_none());
    }

    #[test]
    fn execution_ipc_dto_serializes_opaque_ids_as_decimal_strings() {
        let unsafe_id = 9_007_199_254_740_993_u64;
        let basis = CompilationBasis {
            graph_revision: crate::node_system::document::GraphRevision::new(unsafe_id),
            registry_fingerprint: RegistryFingerprint::from_bytes([2; 32]),
            resource_versions: Default::default(),
        };
        let correlation = CorrelationContext {
            project_session_id: ProjectSessionId::new("session"),
            graph_path: crate::node_system::document::GraphResourcePath("events/main".into()),
            graph_revision: basis.graph_revision,
            registry_fingerprint: basis.registry_fingerprint.clone(),
            resource_versions: basis.resource_versions.clone(),
            compile_id: CompileId::new(unsafe_id),
            selection_digest: Some("demand-selection-a".into()),
            run_id: Some(RunId::new(unsafe_id)),
            node_id: None,
            node_type_id: None,
            parent_call: Some(ParentCallId::new(unsafe_id)),
        };
        let operation = crate::commands::node_system_execution_dto::RunEventDto::from(RunEvent {
            correlation: correlation.clone(),
            basis: basis.clone(),
            kind: RunEventKind::OperationStarted {
                operation_index: 3,
                activation_id: unsafe_id,
            },
        });
        let preview = crate::commands::node_system_execution_dto::RunEventDto::from(RunEvent {
            correlation: CorrelationContext {
                selection_digest: Some("demand-selection-b".into()),
                ..correlation.clone()
            },
            basis: basis.clone(),
            kind: RunEventKind::RunStarted,
        });
        let source =
            ResultSourceDescriptorDto::from(crate::node_system::runtime::ResultSourceDescriptor {
                source_id: ResultSourceId::new(unsafe_id),
                artifact_id: crate::node_system::runtime::ArtifactId::new(unsafe_id),
                name: "result".into(),
                kind: crate::node_system::runtime::ArtifactSnapshotKind::Value,
                total_count: 1,
                correlation: correlation.clone(),
                basis: basis.clone(),
            });
        let result = crate::commands::node_system_execution_dto::RunEventDto::from(RunEvent {
            correlation,
            basis,
            kind: RunEventKind::ResultReady {
                name: "result".into(),
                source_id: ResultSourceId::new(unsafe_id),
            },
        });

        let operation = serde_json::to_value(operation).unwrap();
        assert_eq!(
            operation["correlation"]["graphRevision"],
            unsafe_id.to_string()
        );
        assert_eq!(operation["correlation"]["compileId"], unsafe_id.to_string());
        assert_eq!(
            operation["correlation"]["selectionDigest"],
            "demand-selection-a"
        );
        assert_eq!(operation["correlation"]["runId"], unsafe_id.to_string());
        assert_eq!(
            operation["correlation"]["parentCall"],
            unsafe_id.to_string()
        );
        assert_eq!(operation["basis"]["graphRevision"], unsafe_id.to_string());
        assert_eq!(operation["kind"]["activationId"], unsafe_id.to_string());
        assert!(operation["correlation"].get("graph_revision").is_none());
        assert!(operation["correlation"].get("selection_digest").is_none());
        let preview = serde_json::to_value(preview).unwrap();
        assert_eq!(
            operation["correlation"]["compileId"],
            preview["correlation"]["compileId"]
        );
        assert_ne!(
            operation["correlation"]["selectionDigest"],
            preview["correlation"]["selectionDigest"]
        );
        let source = serde_json::to_value(source).unwrap();
        assert_eq!(
            source["correlation"]["selectionDigest"],
            "demand-selection-a"
        );
        let result = serde_json::to_value(result).unwrap();
        assert_eq!(
            result["correlation"]["selectionDigest"],
            "demand-selection-a"
        );
        assert_eq!(result["kind"]["sourceId"], unsafe_id.to_string());
        let execute_result = serde_json::to_value(ExecuteGraphResultDto {
            run_id: unsafe_id.to_string(),
        })
        .unwrap();
        assert_eq!(execute_result["runId"], unsafe_id.to_string());
        assert_eq!(
            parse_opaque_u64("sourceId", &unsafe_id.to_string()).unwrap(),
            unsafe_id,
        );
        assert_eq!(
            parse_opaque_u64("runId", "not-decimal").unwrap_err().code,
            "invalid_opaque_id",
        );
    }

    #[test]
    fn localized_catalog_rejects_stale_project_identity() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-localized-catalog-stale-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let stale = state.capture_project_session().unwrap().instance_id;
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());

        let error = get_localized_node_catalog_from_state(&state, stale, "en-US").unwrap_err();

        assert_eq!(error.code, "catalog_project_stale");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn localized_catalog_returns_coherent_metadata_with_camel_case_serialization() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-localized-catalog-metadata-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let expected_fingerprint = state
            .project_store
            .read()
            .unwrap()
            .node_registry
            .fingerprint()
            .to_string();

        let catalog =
            get_localized_node_catalog_from_state(&state, project_instance_id.clone(), "en-US")
                .unwrap();

        assert_eq!(
            catalog.project_instance_id.as_ref(),
            project_instance_id.as_str()
        );
        assert_eq!(catalog.registry_fingerprint.as_ref(), expected_fingerprint);
        assert_eq!(catalog.resource_publication_revision, 0);
        let value = serde_json::to_value(&catalog).unwrap();
        assert_eq!(value["projectInstanceId"], project_instance_id.as_str());
        assert_eq!(value["registryFingerprint"], expected_fingerprint);
        assert_eq!(value["resourcePublicationRevision"], 0);
        assert!(value.get("project_instance_id").is_none());
        assert!(value.get("registry_fingerprint").is_none());
        assert!(value.get("resource_publication_revision").is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn localized_catalog_returns_resources_from_the_same_coherent_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-localized-catalog-resource-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let function_path =
            GraphResourcePath::new("functions/Sales Report.yssbi-function").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            function_path.clone(),
            GraphResourceDocument::new("Sales Report", GraphDocumentKind::Function),
        );
        fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &function_path).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let snapshot = state.catalog_snapshot(&project_instance_id).unwrap();
        let expected_fingerprint = snapshot.registry.fingerprint().to_string();
        let expected_revision = snapshot.resource_publication_revision;

        let catalog =
            get_localized_node_catalog_from_state(&state, project_instance_id.clone(), "zh-CN")
                .unwrap();

        assert_eq!(
            catalog.project_instance_id.as_ref(),
            project_instance_id.as_str()
        );
        assert_eq!(catalog.registry_fingerprint.as_ref(), expected_fingerprint);
        assert_eq!(catalog.resource_publication_revision, expected_revision);
        let resource = catalog
            .items
            .iter()
            .find(|item| item.resource_path.is_some())
            .expect("persisted function must be projected by the Catalog command");
        assert_eq!(resource.title.as_ref(), "Sales Report");
        assert_eq!(
            resource
                .resource_path
                .as_ref()
                .map(crate::node_system::catalog::CatalogResourcePath::as_str),
            Some(function_path.as_str())
        );
        assert!(matches!(
            resource.creation,
            NodeCreationDescriptor::ResourceBound { .. }
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn run_result_routes_canonical_resource_mutation_without_split_reconstruction() {
        let run_source = include_str!("../node_system/runtime/run.rs");
        let project_source = include_str!("../project/project_state.rs");
        let command_source = include_str!("command_node_system.rs");
        let resource_source = include_str!("../node_system/runtime/project_resource.rs");
        let reconstruction_helper = ["variable_effect_", "mutation_result"].concat();
        let publication_helper = ["publish_run_", "variable_effects"].concat();

        assert!(
            run_source
                .contains("pub resource_mutation: Option<crate::event::ResourceMutationResultDto>")
        );
        for split_field in [
            "resource_project_instance_id",
            "resource_publication_revision",
            "resource_deltas",
            "resource_history",
        ] {
            assert!(!run_source.contains(split_field));
        }
        assert!(!project_source.contains(&reconstruction_helper));
        assert!(!command_source.contains(&reconstruction_helper));
        assert!(!command_source.contains(&publication_helper));
        assert!(
            command_source
                .contains("publish_run_resource_mutation(result.resource_mutation.as_ref()")
        );
        assert!(!resource_source.contains(
            "does not support Run-side writes until durable revisioned commits are available"
        ));
    }

    #[test]
    fn project_path_writes_are_confined_to_activation_publication() {
        let project_source = include_str!("../project/project_state.rs");
        assert!(!project_source.contains("pub project_path:"));
        assert!(!project_source.contains("fn set_path("));

        let writes = project_source
            .match_indices("std::mem::replace(&mut *current_path, path)")
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();
        assert_eq!(writes.len(), 1, "project path must have one mutation site");

        let publish_start = project_source
            .find("    pub(super) fn publish_project_activation(")
            .expect("activation publication must exist");
        let publish_end = project_source[publish_start..]
            .find("\n    pub fn get_path(")
            .map(|offset| publish_start + offset)
            .expect("activation publication region must be isolated");
        assert!(writes[0] > publish_start && writes[0] < publish_end);
    }

    #[test]
    fn activation_final_publication_uses_only_constant_time_authority_checks() {
        let project_source = include_str!("../project/project_state.rs");
        let publish_start = project_source
            .find("    pub(super) fn publish_project_activation(")
            .expect("activation publication must exist");
        let publish_end = project_source[publish_start..]
            .find("\n    pub fn get_path(")
            .map(|offset| publish_start + offset)
            .expect("activation publication region must be isolated");
        let publish = &project_source[publish_start..publish_end];

        assert!(publish.contains("authority_generation"));
        for forbidden in [
            "canonical_semantic_value",
            "serde_json::to_value",
            "current_data.clone()",
            "current_store.clone()",
            ".sort_unstable()",
            ".clear();",
            ".retain(",
            "*current_data =",
            "*current_store =",
            "*current_graph_revisions =",
            "*current_variable_revisions =",
            "*current_worksheet_revisions =",
            "*history =",
        ] {
            assert!(
                !publish.contains(forbidden),
                "activation publication contains size-dependent work: {forbidden}"
            );
        }
    }

    #[test]
    fn filesystem_publication_callers_capture_projection_environment_before_commit() {
        let project_source = include_str!("../project/project_state.rs");
        for (caller, start, end) in [
            (
                "rename",
                "    pub(super) fn rename_graph_resource_transaction_impl(",
                "\n    fn graph_rename_mutations(",
            ),
            (
                "worksheet upsert",
                "    pub fn upsert_worksheet_document(",
                "\n    pub fn remove_worksheet_document(",
            ),
            (
                "worksheet removal",
                "    pub fn remove_worksheet_document(",
                "\n    pub(super) fn allocate_graph_path_from_snapshot(",
            ),
        ] {
            let caller_start = project_source
                .find(start)
                .unwrap_or_else(|| panic!("{caller} caller must exist"));
            let caller_end = project_source[caller_start..]
                .find(end)
                .map(|offset| caller_start + offset)
                .unwrap_or_else(|| panic!("{caller} caller region must be isolated"));
            let source = &project_source[caller_start..caller_end];
            let capture = source
                .find("capture_projection_environment_for_session(")
                .unwrap_or_else(|| panic!("{caller} must capture projection environment"));
            let commit = source
                .find("prepared.commit()")
                .unwrap_or_else(|| panic!("{caller} must commit a prepared filesystem mutation"));
            assert!(
                capture < commit,
                "{caller} captures projection environment after filesystem commit"
            );
            assert!(source.contains("apply_resource_document_patch_with_environment("));
            assert!(!source.contains("self.apply_resource_document_patch("));
        }
    }

    #[test]
    fn recovery_mutation_conflict_preserves_stable_app_error_code() {
        let error = mutation_conflict_to_app_error(
            crate::node_system::document::MutationConflict::RecoveryRequired(
                "project requires recovery".into(),
            ),
            "graph_revision_conflict",
        );

        assert_eq!(error.code, "project_recovery_required");
        assert_eq!(
            error.details,
            Some(serde_json::json!({ "recoveryRequired": true }))
        );
    }

    #[test]
    fn malformed_create_node_body_maps_to_catalog_descriptor_invalid() {
        let raw = serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000777",
            "payload": {
                "type": "createNode",
                "payload": {
                    "descriptor": {
                        "kind": "resourceBound",
                        "nodeTypeId": "yssbi.project.function.call",
                        "resourcePath": "functions/Helper.yssbi-function",
                        "resourceRevision": 0,
                        "createArgs": { "kind": "function" }
                    },
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": null,
                    "parameters": { "target": "functions/Injected.yssbi-function" }
                }
            }
        });

        let malformed_descriptor = serde_json::json!({
            "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000779",
            "payload": {
                "type": "createNode",
                "payload": {
                    "descriptor": {
                        "kind": "resourceBound",
                        "nodeTypeId": "yssbi.project.function.call",
                        "resourcePath": "functions/Helper.yssbi-function",
                        "resourceRevision": "stale",
                        "createArgs": { "kind": "function" }
                    },
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": null
                }
            }
        });

        for request in [raw, malformed_descriptor] {
            let error = parse_editor_mutation_request(request).unwrap_err();
            assert_eq!(error.code, "catalog_descriptor_invalid");
        }
    }

    #[test]
    fn non_descriptor_request_shape_errors_are_not_catalog_errors() {
        let valid_static_descriptor = serde_json::json!({
            "kind": "static",
            "nodeTypeId": "yssbi.constant.int64"
        });
        let cases = [
            serde_json::json!({
                "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
                "baseRevision": 0,
                "operationId": "00000000-0000-0000-0000-000000000801",
                "payload": { "type": "moveNodes", "payload": { "positions": "invalid" } }
            }),
            serde_json::json!({
                "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
                "baseRevision": 0,
                "operationId": "00000000-0000-0000-0000-000000000802",
                "payload": {
                    "type": "connect",
                    "payload": { "output": { "kind": "declared" }, "input": null, "order": null }
                }
            }),
            serde_json::json!({
                "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
                "baseRevision": 0,
                "operationId": "not-an-operation-id",
                "payload": {
                    "type": "createNode",
                    "payload": {
                        "descriptor": valid_static_descriptor.clone(),
                        "position": { "x": 1.0, "y": 2.0 },
                        "userLabel": null
                    }
                }
            }),
            serde_json::json!({
                "resource": { "kind": "graph", "key": 7 },
                "baseRevision": "zero",
                "operationId": "00000000-0000-0000-0000-000000000803",
                "payload": { "type": "deleteNode", "payload": { "nodeId": "invalid" } }
            }),
            serde_json::json!({
                "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
                "baseRevision": 0,
                "operationId": "00000000-0000-0000-0000-000000000804",
                "payload": {
                    "type": "createNode",
                    "payload": {
                        "descriptor": valid_static_descriptor.clone(),
                        "position": { "x": "left", "y": 2.0 },
                        "userLabel": null
                    }
                }
            }),
            serde_json::json!({
                "resource": { "kind": "graph", "key": "events/Main.yssbi-event" },
                "baseRevision": 0,
                "operationId": "00000000-0000-0000-0000-000000000805",
                "payload": {
                    "type": "createNode",
                    "payload": {
                        "descriptor": valid_static_descriptor,
                        "position": { "x": 1.0, "y": 2.0 },
                        "userLabel": 42
                    }
                }
            }),
        ];

        for raw in cases {
            let error = parse_editor_mutation_request(raw).unwrap_err();
            assert_eq!(error.code, "invalid_editor_mutation");
        }
    }

    #[test]
    fn injected_create_node_command_has_zero_authoritative_effects() {
        let state = ProjectState::new();
        let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        state
            .insert_graph(
                graph_path.clone(),
                GraphResourceDocument::new("Main", GraphDocumentKind::Event),
            )
            .unwrap();
        let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
        let history_before = state.history_status();
        let revisions_before = state.revision_state_for_test();
        let publication_before = state.publication_state_for_test();
        let raw = serde_json::json!({
            "resource": { "kind": "graph", "key": graph_path.as_str() },
            "baseRevision": 0,
            "operationId": "00000000-0000-0000-0000-000000000778",
            "payload": {
                "type": "createNode",
                "payload": {
                    "descriptor": {
                        "kind": "resourceBound",
                        "nodeTypeId": "yssbi.project.function.call",
                        "resourcePath": "functions/Helper.yssbi-function",
                        "resourceRevision": 0,
                        "createArgs": { "kind": "function" }
                    },
                    "position": { "x": 1.0, "y": 2.0 },
                    "userLabel": null,
                    "parameters": { "target": "functions/Injected.yssbi-function" }
                }
            }
        });
        let mut events = Vec::new();

        let error = mutate_graph_document_with_emitter(
            &state,
            graph_path.as_str().to_string(),
            "en-US",
            raw,
            |event| events.push(event),
        )
        .unwrap_err();

        assert_eq!(error.code, "catalog_descriptor_invalid");
        assert!(events.is_empty());
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            data_before
        );
        assert_eq!(state.history_status(), history_before);
        assert_eq!(state.revision_state_for_test(), revisions_before);
        assert_eq!(state.publication_state_for_test(), publication_before);
    }

    #[test]
    fn catalog_mutation_conflicts_preserve_stable_app_error_codes() {
        for (conflict, expected) in [
            (
                crate::node_system::document::MutationConflict::CatalogResourceStale(
                    "resource changed".into(),
                ),
                "catalog_resource_stale",
            ),
            (
                crate::node_system::document::MutationConflict::CatalogDescriptorInvalid(
                    "descriptor is invalid".into(),
                ),
                "catalog_descriptor_invalid",
            ),
        ] {
            let error = mutation_conflict_to_app_error(conflict, "graph_revision_conflict");
            assert_eq!(error.code, expected);
        }
    }

    #[test]
    fn run_variable_effects_publish_only_resource_mutation_committed() {
        let delta = ResourceDeltaEvent {
            resource: ResourceKey::Variable(VariableResourceKey(
                "variables/00000000-0000-0000-0000-000000000701".into(),
            )),
            from_revision: ResourceRevision::INITIAL,
            to_revision: ResourceRevision::new(1),
            caused_by: None,
            payload: ResourceDocumentPatch::Variable(VariableDocumentPatch::new(
                Some(serde_json::json!({ "Int64": 1 })),
                Some(serde_json::json!({ "Int64": 2 })),
            )),
        };
        let mut events = Vec::new();

        let result = ResourceMutationResultDto {
            operation_id: OperationId::new(),
            project_instance_id: "00000000-0000-0000-0000-000000000601".into(),
            publication_revision: 7,
            moves: Vec::new(),
            deltas: vec![delta],
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: crate::event::ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history: HistoryStatusDto {
                can_undo: true,
                can_redo: false,
            },
        };
        publish_run_resource_mutation(Some(&result), |event| events.push(event));

        assert_eq!(events.len(), 1);
        let Event::Project(EventProject::ResourceMutationCommitted { result: emitted }) =
            &events[0]
        else {
            panic!("run publication must emit one canonical resource mutation");
        };
        assert_eq!(emitted, &result);
    }

    #[test]
    fn save_command_preserves_identity_revision_operation_and_emits_once() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-save-command-contract-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Main", GraphDocumentKind::Event),
        );
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let operation_id = crate::node_system::document::OperationId::new();
        let mut events = Vec::new();

        let result = save_project_graph_with_emitter(
            &state,
            project_instance_id.clone(),
            path,
            ResourceRevision::INITIAL,
            operation_id,
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(result.project_instance_id, project_instance_id.as_str());
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            Event::Project(EventProject::ProjectSaved { result: emitted }) if emitted == &result
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_save_command_emits_no_event() {
        let root =
            std::env::temp_dir().join(format!("yssbi-stale-save-command-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Main", GraphDocumentKind::Event),
        );
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let stale = state.capture_project_session().unwrap().instance_id;
        state.activate_project_fixture(
            root.to_string_lossy().into_owned(),
            state.get_data().unwrap(),
        );
        let mut events = Vec::new();

        let error = save_project_graph_with_emitter(
            &state,
            stale,
            path,
            ResourceRevision::INITIAL,
            crate::node_system::document::OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();

        assert_eq!(error.code, "stale_project_lifecycle");
        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_command_rejects_stale_project_before_registration_io_or_event() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-stale-rename-command-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let old_path = GraphResourcePath::new("events/Old.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            old_path.clone(),
            GraphResourceDocument::new("Old", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        crate::project::fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &old_path)
            .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        std::fs::write(root.join(old_path.as_str()), b"malformed graph").unwrap();
        let mut events = Vec::new();

        let error = rename_graph_resource_with_emitter(
            &state,
            ProjectInstanceId::from_existing("stale-project-instance".into()),
            old_path.clone(),
            ResourceRevision::INITIAL,
            "New",
            1,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();

        assert_eq!(state.graph_lifecycle_entry_count(), 0);
        assert!(events.is_empty());
        assert!(root.join(old_path.as_str()).exists());
        assert!(!root.join("events/New.yssbi-event").exists());
        assert_eq!(error.code, "stale_project_lifecycle");
        assert!(error.message.contains("stale project lifecycle"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_command_preserves_recovery_required_code_and_emits_nothing() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-recovery-rename-command-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let old_path = GraphResourcePath::new("events/Old.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            old_path.clone(),
            GraphResourceDocument::new("Old", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        crate::project::fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &old_path)
            .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        state
            .project_recovery_marker()
            .mark("unwind rollback failed");
        let project_instance_id = state.project_instance_id();
        let mut events = Vec::new();

        let error = rename_graph_resource_with_emitter(
            &state,
            ProjectInstanceId::from_existing(project_instance_id.clone()),
            old_path.clone(),
            ResourceRevision::INITIAL,
            "New",
            1,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();

        assert_eq!(error.code, "project_recovery_required");
        assert_eq!(
            error.details,
            Some(serde_json::json!({ "recoveryRequired": true }))
        );
        assert!(events.is_empty());
        assert!(root.join(old_path.as_str()).exists());
        assert!(!root.join("events/New.yssbi-event").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn resource_command_emitter_failure_preserves_committed_receipt_observability() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-resource-command-emitter-failure-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        crate::project::fixtures::write_project(
            &ProjectData::new(),
            root.to_string_lossy().as_ref(),
        )
        .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_id = state.capture_project_session().unwrap().instance_id;
        let operation_id = OperationId::new();

        let result = create_graph_resource_with_emitter(
            &state,
            project_id.clone(),
            "Committed",
            GraphDocumentKind::Event,
            operation_id,
            |_| Err::<(), _>("emitter offline"),
        )
        .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.project_instance_id, project_id.as_str());
        assert!(root.join("events/Committed.yssbi-event").is_file());
        let replay = state
            .create_graph_resource_transaction(
                &project_id,
                "Committed",
                GraphDocumentKind::Event,
                operation_id,
            )
            .unwrap_err();
        assert_eq!(replay.code(), "duplicate_operation");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resource_commands_emit_one_project_scoped_committed_result() {
        let source = include_str!("command_node_system.rs");

        for required in [
            "fn create_graph_resource_with_emitter<R: EmitOutcome>(",
            "fn duplicate_graph_resource_with_emitter<R: EmitOutcome>(",
            "fn remove_graph_resource_with_emitter<R: EmitOutcome>(",
            "fn rename_graph_resource_with_emitter<R: EmitOutcome>(",
            "project_instance_id: ProjectInstanceId",
            "expected_revision: ResourceRevision",
            "lifecycle_token: u64",
            "operation_id: OperationId",
        ] {
            assert!(
                source.contains(required),
                "resource command contract is missing {required}"
            );
        }
        let resource_commands = &source[source.find("pub fn create_event(").unwrap()
            ..source.find("pub fn update_function_signature(").unwrap()];
        assert_eq!(
            source
                .matches("\n    emit_resource_result(&mut emit, &result);")
                .count(),
            4,
            "each resource command helper must emit through the canonical helper"
        );
        assert_eq!(
            resource_commands
                .matches("EventProject::ResourceMutationCommitted")
                .count(),
            0,
            "resource commands must not construct a second event path"
        );
        assert!(!resource_commands.contains("GraphResourceMoved"));
        assert!(
            !resource_commands.contains("emit_project_index_invalidated(&app, \"remove_graph\")")
        );

        let create_root = std::env::temp_dir().join(format!(
            "yssbi-resource-command-create-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&create_root).unwrap();
        crate::project::fixtures::write_project(
            &ProjectData::new(),
            create_root.to_string_lossy().as_ref(),
        )
        .unwrap();
        let create_state = ProjectState::new();
        create_state.activate_project_fixture(
            create_root.to_string_lossy().into_owned(),
            ProjectData::new(),
        );
        let create_id = create_state.capture_project_session().unwrap().instance_id;
        let mut create_events = Vec::new();
        let create_operation_id = OperationId::new();
        let created = create_graph_resource_with_emitter(
            &create_state,
            create_id.clone(),
            "Created",
            GraphDocumentKind::Event,
            create_operation_id,
            |event| create_events.push(event),
        )
        .unwrap();
        assert_eq!(created.operation_id, create_operation_id);
        assert_eq!(created.project_instance_id, create_id.as_str());
        assert_eq!(created.deltas.len(), 1);
        assert_eq!(
            created.deltas[0].resource,
            ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                "events/Created.yssbi-event".into(),
            ))
        );
        assert_eq!(created.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(created.deltas[0].to_revision, ResourceRevision::new(1));
        assert_eq!(created.deltas[0].caused_by, Some(create_operation_id));
        assert_eq!(
            serde_json::to_value(&created.deltas[0].payload).unwrap(),
            serde_json::json!({
                "kind": "graph_resource_lifecycle",
                "patch": {
                    "before": null,
                    "after": {
                        "revision": 0,
                        "path": "events/Created.yssbi-event",
                        "kind": "event"
                    }
                }
            })
        );
        assert!(matches!(
            create_events.as_slice(),
            [Event::Project(EventProject::ResourceMutationCommitted { result })]
                if result == &created
        ));

        for operation in ["duplicate", "remove", "rename"] {
            let root = std::env::temp_dir().join(format!(
                "yssbi-resource-command-{operation}-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let path = GraphResourcePath::new("events/Source.yssbi-event").unwrap();
            let mut data = ProjectData::new();
            data.graphs.insert(
                path.clone(),
                GraphResourceDocument::new("Source", GraphDocumentKind::Event),
            );
            crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref())
                .unwrap();
            crate::project::fixtures::write_graph(&data, root.to_string_lossy().as_ref(), &path)
                .unwrap();
            let state = ProjectState::new();
            state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
            let project_id = state.capture_project_session().unwrap().instance_id;
            let mut events = Vec::new();
            let operation_id = OperationId::new();
            let result = match operation {
                "duplicate" => duplicate_graph_resource_with_emitter(
                    &state,
                    project_id.clone(),
                    path,
                    ResourceRevision::INITIAL,
                    operation_id,
                    |event| events.push(event),
                ),
                "remove" => remove_graph_resource_with_emitter(
                    &state,
                    project_id.clone(),
                    path,
                    ResourceRevision::INITIAL,
                    operation_id,
                    |event| events.push(event),
                ),
                "rename" => rename_graph_resource_with_emitter(
                    &state,
                    project_id.clone(),
                    path,
                    ResourceRevision::INITIAL,
                    "Renamed",
                    1,
                    operation_id,
                    |event| events.push(event),
                ),
                _ => unreachable!(),
            }
            .unwrap();
            assert_eq!(result.operation_id, operation_id);
            assert_eq!(result.project_instance_id, project_id.as_str());
            if operation != "rename" {
                assert_eq!(
                    result.deltas.len(),
                    1,
                    "{operation} must emit one lifecycle delta"
                );
                let delta = &result.deltas[0];
                assert_eq!(delta.from_revision, ResourceRevision::INITIAL);
                assert_eq!(delta.to_revision, ResourceRevision::new(1));
                assert_eq!(delta.caused_by, Some(operation_id));
                let expected_path = if operation == "remove" {
                    "events/Source.yssbi-event"
                } else {
                    "events/Source 1.yssbi-event"
                };
                assert_eq!(
                    delta.resource,
                    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                        expected_path.into(),
                    ))
                );
                let state = serde_json::json!({
                    "revision": 0,
                    "path": expected_path,
                    "kind": "event"
                });
                let (before, after) = if operation == "remove" {
                    (state, serde_json::Value::Null)
                } else {
                    (serde_json::Value::Null, state)
                };
                assert_eq!(
                    serde_json::to_value(&delta.payload).unwrap(),
                    serde_json::json!({
                        "kind": "graph_resource_lifecycle",
                        "patch": { "before": before, "after": after }
                    })
                );
            }
            assert!(matches!(
                events.as_slice(),
                [Event::Project(EventProject::ResourceMutationCommitted { result: emitted })]
                    if emitted == &result
            ));
            std::fs::remove_dir_all(root).unwrap();
        }
        std::fs::remove_dir_all(create_root).unwrap();
    }

    #[test]
    fn rename_command_returns_and_emits_canonical_mutation_result() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-rename-command-event-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let old_path = GraphResourcePath::new("events/Old.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            old_path.clone(),
            GraphResourceDocument::new("Old", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let mut events = Vec::new();
        let project_instance_id = state.project_instance_id();

        let result = rename_graph_resource_with_emitter(
            &state,
            ProjectInstanceId::from_existing(project_instance_id.clone()),
            old_path.clone(),
            ResourceRevision::INITIAL,
            "New",
            1,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(result.project_instance_id, project_instance_id);
        assert_eq!(result.publication_revision, 1);
        assert_eq!(result.moves.len(), 1);
        assert_eq!(result.moves[0].from, old_path.as_str());
        assert_eq!(result.moves[0].to, "events/New.yssbi-event");
        assert_eq!(result.deltas.len(), 1);
        assert_eq!(
            result.deltas[0].resource,
            ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                "events/New.yssbi-event".into()
            ))
        );
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::new(1));
        assert!(result.deltas[0].caused_by.is_some());
        assert!(result.projection_replacements.is_empty());
        assert_eq!(
            result.projection_status,
            crate::event::ProjectionStatusDto::Incomplete {
                invalidated_graph_paths: vec![
                    "events/New.yssbi-event".into(),
                    old_path.as_str().to_string(),
                ],
            }
        );
        assert!(result.history.can_undo);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            Event::Project(EventProject::ResourceMutationCommitted { result: emitted })
                if emitted == &result
        ));
        let _ = std::fs::remove_dir_all(root);
    }
}
