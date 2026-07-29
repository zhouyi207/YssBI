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
use crate::node_system::runtime::{
    ArtifactSnapshot, ResultSourceDescriptor, ResultSourceId, ResultSourcePage, RunEvent,
    RunEventSink,
};
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{GraphResourcePath, ProjectInstanceId, ProjectState};
use serde::Serialize;
use tauri::{AppHandle, State, ipc::Channel};

fn parse_graph_path(value: String) -> Result<GraphResourcePath, AppError> {
    GraphResourcePath::new(value).map_err(AppError::from)
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
        crate::node_system::document::MutationConflict::StaleRevision { .. } => {
            AppError::new(revision_conflict_code, error.to_string())
        }
        _ => AppError::internal(error),
    }
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

#[tauri::command]
pub fn mutate_graph_document(
    app: AppHandle,
    state: State<'_, ProjectState>,
    graph_path: String,
    locale: String,
    request: MutationRequest<EditorGraphMutationDto>,
) -> Result<GraphMutationResultDto, AppError> {
    state
        .apply_editor_graph_mutation_observed(
            &parse_graph_path(graph_path)?,
            &locale,
            request,
            |delta| {
                emit_project_event(
                    &app,
                    Event::Project(EventProject::GraphDelta {
                        delta: delta.clone(),
                    }),
                )
            },
        )
        .map_err(|error| mutation_conflict_to_app_error(error, "graph_revision_conflict"))
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
) -> Result<Option<ResultSourceDescriptor>, AppError> {
    state
        .result_source_descriptor(ResultSourceId::new(source_id))
        .map_err(AppError::from)
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
) -> Result<Option<ResultSourceValueDto>, AppError> {
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
    source_id: u64,
    offset: usize,
    limit: usize,
) -> Result<Option<ResultSourcePage>, AppError> {
    state
        .result_source_page(ResultSourceId::new(source_id), offset, limit)
        .map_err(AppError::from)
}

#[tauri::command]
pub fn release_result_source(
    state: State<'_, ProjectState>,
    source_id: u64,
) -> Result<bool, AppError> {
    state
        .release_result_source(ResultSourceId::new(source_id))
        .map_err(AppError::from)
}

#[tauri::command]
pub fn release_run_result_sources(
    state: State<'_, ProjectState>,
    run_id: u64,
) -> Result<usize, AppError> {
    state
        .release_run_result_sources(crate::node_system::analysis::RunId::new(run_id))
        .map_err(AppError::from)
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
                publish_run_resource_mutation(result.resource_mutation.as_ref(), |event| {
                    emit_project_event(&app, event)
                });
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
        ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey, ResourceRevision,
        VariableDocumentPatch, VariableResourceKey,
    };
    use crate::project::{GraphDocumentKind, GraphResourceDocument, ProjectData};

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
    fn committed_resource_completion_source_is_total_and_state_independent() {
        let project_source = include_str!("../project/project_state.rs");
        let receipt_start = project_source
            .find("impl CommittedResourceMutation {")
            .expect("committed receipt completion impl must exist");
        let receipt_end = project_source[receipt_start..]
            .find("\nimpl ProjectState {")
            .map(|offset| receipt_start + offset)
            .expect("receipt completion impl must end before ProjectState impl");
        let completion = &project_source[receipt_start..receipt_end];

        assert!(completion.contains(
            "fn complete(self, locale: &str) -> crate::event::ResourceMutationResultDto"
        ));
        for forbidden in [
            "Result<",
            "ensure_mutation_operational",
            "ensure_project_operational",
            "self.project_",
            "self.history",
            "self.mutation_publication",
            "std::fs",
            "ProjectFilesystem",
        ] {
            assert!(
                !completion.contains(forbidden),
                "committed completion contains forbidden post-receipt dependency: {forbidden}"
            );
        }

        let projection_start = project_source
            .find("impl ProjectionSourceSnapshot {")
            .expect("projection snapshot impl must exist");
        let projection_end = project_source[projection_start..]
            .find("\npub struct ProjectState {")
            .map(|offset| projection_start + offset)
            .expect("projection snapshot impl must end before ProjectState");
        let projection = &project_source[projection_start..projection_end];
        assert!(projection.contains("compile_resources_from_projection_snapshot(self)"));
        for forbidden in [
            "read_table_meta",
            "crate::database",
            "std::fs",
            "ProjectState",
        ] {
            assert!(
                !projection.contains(forbidden),
                "receipt projection snapshot contains forbidden live dependency: {forbidden}"
            );
        }

        let compile_start = project_source
            .find("fn compile_resources_from_projection_snapshot(")
            .expect("projection compile helper must exist");
        let compile_end = project_source[compile_start..]
            .find("\npub(super) fn snapshot_project_resources(")
            .map(|offset| compile_start + offset)
            .expect("projection compile helper must have an isolated source region");
        let compile_helper = &project_source[compile_start..compile_end];
        for forbidden in [
            "read_table_meta",
            "crate::database",
            "project_root_from_path",
            "std::fs",
        ] {
            assert!(
                !compile_helper.contains(forbidden),
                "receipt projection compile helper contains forbidden live dependency: {forbidden}"
            );
        }

        for forbidden_api in [
            "pub fn update_function_signature(",
            "pub fn undo_last_transaction(",
            "pub fn redo_last_transaction(",
            "resource_project_instance_id",
            "resource_publication_revision",
            "resource_deltas",
            "resource_history",
        ] {
            assert!(
                !project_source.contains(forbidden_api),
                "resource publication retains split/delta-only API: {forbidden_api}"
            );
        }

        assert!(!project_source.contains("fn complete_resource_mutation("));
        assert!(!project_source.contains("let data = self\n            .get_data()"));
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
    fn projection_environment_capture_rejects_mixed_activation_generation() {
        let project_source = include_str!("../project/project_state.rs");
        assert!(
            project_source.contains("activation_generation: Arc<std::sync::atomic::AtomicU64>")
        );
        assert!(project_source.contains("activation_identity:"));
        assert!(!project_source.contains(".capture_projection_environment()"));

        let publish_start = project_source
            .find("    pub(super) fn publish_project_activation(")
            .expect("activation publication must exist");
        let publish_end = project_source[publish_start..]
            .find("\n    pub fn get_path(")
            .map(|offset| publish_start + offset)
            .expect("activation publication region must be isolated");
        let publish = &project_source[publish_start..publish_end];
        let store_guard = publish
            .find("let (mut current_store, store_recovered)")
            .expect("activation must retain the recovered runtime-store guard");
        let changing = publish
            .find("ActivationGenerationTransition::begin")
            .expect("activation must mark generation changing through RAII");
        let path_install = publish
            .find("std::mem::replace(&mut *current_path, path)")
            .expect("activation must install project path");
        let store_install = publish
            .find("std::mem::replace(&mut *current_store, store)")
            .expect("activation must install project store through the named guard");
        let stable = publish
            .rfind("generation.complete();")
            .expect("activation must mark generation stable");
        assert!(store_guard < changing);
        assert!(changing < path_install);
        assert!(path_install < store_install);
        assert!(store_install < stable);

        let capture_start = project_source
            .find("    fn capture_projection_environment(")
            .expect("projection environment capture must exist");
        let capture_end = project_source[capture_start..]
            .find("\n    fn projection_source_snapshot(")
            .map(|offset| capture_start + offset)
            .expect("projection environment capture region must be isolated");
        let capture = &project_source[capture_start..capture_end];
        assert!(capture.contains("expected: &ProjectionEnvironmentExpectation"));
        assert!(capture.contains("activation_generation.load"));
        assert!(capture.contains("generation_before != generation_after"));
        assert!(capture.contains("generation_after % 2 != 0"));
        assert!(capture.contains("stale_project_lifecycle"));
        assert!(capture.contains("databases.contains_key(id)"));
        let data_drop = capture.find("drop(data);").unwrap();
        let path_drop = capture.find("drop(path);").unwrap();
        let overlap_hook = capture
            .find("run_projection_environment_after_path_data_test_hook")
            .expect("capture must expose deterministic post-path/data overlap hook");
        let store_lock = capture
            .find("let store = self.project_store.read().unwrap();")
            .expect("capture must snapshot cached store schemas");
        let metadata_io = capture
            .find("crate::database::read_table_meta")
            .expect("capture must materialize uncached metadata after locks");
        let final_recheck = capture
            .rfind("activation_generation.load")
            .expect("capture must recheck generation after metadata I/O");
        assert!(data_drop < overlap_hook);
        assert!(path_drop < overlap_hook);
        assert!(overlap_hook < store_lock);
        assert!(store_lock < metadata_io);
        assert!(metadata_io < final_recheck);

        for (caller, end) in [
            (
                "    fn commit_graph_patch(",
                "\n    pub fn update_function_signature_observed(",
            ),
            (
                "    fn commit_function_signature(",
                "\n    pub fn undo_last_transaction_observed(",
            ),
            (
                "    fn commit_history_direction(",
                "\n    fn commit_variable_effect_history_direction(",
            ),
        ] {
            let start = project_source.find(caller).unwrap();
            let finish = project_source[start..]
                .find(end)
                .map(|offset| start + offset)
                .unwrap();
            let region = &project_source[start..finish];
            assert!(region.contains(
                "let expected_session = self.current_projection_environment_expectation();"
            ));
            assert!(region.contains("capture_projection_environment(&expected_session)"));
        }

        let variable_start = project_source
            .find("    fn commit_variable_effects_receipt(")
            .unwrap();
        let variable_end = project_source[variable_start..]
            .find("fn install_variable_effect_snapshots(")
            .map(|offset| variable_start + offset)
            .unwrap();
        let variable = &project_source[variable_start..variable_end];
        assert!(variable.contains("capture_projection_environment_for_execution_session("));
        assert!(
            variable
                .find("capture_projection_environment_for_execution_session(")
                .unwrap()
                < variable.find(".commit()").unwrap()
        );
    }

    #[test]
    fn projection_environment_capture_lock_order_is_activation_compatible() {
        let project_source = include_str!("../project/project_state.rs");
        let capture_start = project_source
            .find("    fn capture_projection_environment(")
            .expect("projection environment capture must exist");
        let capture_end = project_source[capture_start..]
            .find("\n    fn projection_source_snapshot(")
            .map(|offset| capture_start + offset)
            .expect("capture lock region must be isolated");
        let capture = &project_source[capture_start..capture_end];

        let path_lock = capture
            .find("let path = self.project_path.read().unwrap();")
            .expect("capture must acquire project path first");
        let data_lock = capture
            .find("let data = self.project_data.read().unwrap();")
            .expect("capture must acquire project data second");
        let data_drop = capture
            .find("drop(data);")
            .expect("capture must release project data before materialization");
        let path_drop = capture
            .find("drop(path);")
            .expect("capture must release project path before materialization");
        let materialize = capture
            .find("let project_root = project_path")
            .expect("capture must materialize from owned path and declarations");
        assert!(path_lock < data_lock);
        assert!(data_lock < data_drop);
        assert!(data_drop < path_drop);
        assert!(path_drop < materialize);
        let locked_region = &capture[..path_drop];
        for forbidden in ["mutation_publication", "project_store", "read_table_meta"] {
            assert!(
                !locked_region.contains(forbidden),
                "capture lock region contains forbidden dependency: {forbidden}"
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
