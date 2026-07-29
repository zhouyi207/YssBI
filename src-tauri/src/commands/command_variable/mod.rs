use crate::error::AppError;
use crate::event::ResourceMutationResultDto;
use crate::event::{Event, EventProject, EventVariable, emit_project_event};
use crate::graph::value::{DataType, DataValue};
use crate::node_system::document::{OperationId, ResourceRevision};
#[cfg(test)]
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{ProjectInstanceId, ProjectState};
use crate::schema::VariableInstanceDTO;
use crate::variable::{VariableId, VariableScope};
use tauri::{AppHandle, State};

fn ensure_command_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
) -> Result<(), AppError> {
    let session = state.capture_project_session().map_err(AppError::from)?;
    if &session.instance_id != project_instance_id {
        return Err(AppError::new(
            "stale_project_lifecycle",
            "variable command project instance is stale",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn persist_global_variables_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    expected_revisions: std::collections::BTreeMap<
        crate::node_system::document::ResourceKey,
        crate::node_system::document::ResourceRevision,
    >,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<ProjectSaveResultDto, AppError> {
    let result = state
        .persist_global_variables(&project_instance_id, expected_revisions, operation_id)
        .map_err(AppError::from)?;
    emit(Event::Project(EventProject::ProjectSaved {
        result: result.clone(),
    }));
    Ok(result)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableCommandResult {
    pub variable_id: String,
    pub variable: Option<VariableInstanceDTO>,
    pub result: Option<ResourceMutationResultDto>,
}

fn emit_global_result(emit: &mut impl FnMut(Event), result: &ResourceMutationResultDto) {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.clone(),
    }));
}

fn create_variable_with_emitter(
    state: &ProjectState,
    name: &str,
    data_type: DataType,
    data_value: DataValue,
    description: &str,
    scope: VariableScope,
    tags: Vec<String>,
    project_instance_id: ProjectInstanceId,
    expected_collection_revision: u64,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<VariableCommandResult, AppError> {
    ensure_command_project(state, &project_instance_id)?;
    ensure_variable_data_type(&data_type)?;
    if matches!(scope, VariableScope::Global) {
        let committed = state
            .create_global_variable_transaction(
                &project_instance_id,
                name.to_string(),
                data_type,
                data_value,
                description.to_string(),
                tags,
                expected_collection_revision,
                operation_id,
            )
            .map_err(AppError::from)?;
        emit_global_result(&mut emit, &committed.result);
        return Ok(VariableCommandResult {
            variable_id: committed.variable.id.to_string(),
            variable: Some((&committed.variable).into()),
            result: Some(committed.result),
        });
    }
    let variable = state
        .add_variable(name, data_type, data_value, description, scope, tags)
        .map_err(AppError::from)?;
    emit(Event::Variable(EventVariable::VariableCreated {
        variable_id: variable.id,
        variable_scope: variable.scope.clone(),
        data: (&variable).into(),
    }));
    Ok(VariableCommandResult {
        variable_id: variable.id.to_string(),
        variable: Some((&variable).into()),
        result: None,
    })
}

fn update_variable_with_emitter(
    state: &ProjectState,
    variable_id: VariableId,
    name: Option<String>,
    data_type: Option<DataType>,
    data_value: Option<DataValue>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<VariableCommandResult, AppError> {
    ensure_command_project(state, &project_instance_id)?;
    if let Some(ref data_type) = data_type {
        ensure_variable_data_type(data_type)?;
    }
    let current = state
        .get_variable(&variable_id)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "variable_not_found",
                format!("Variable '{variable_id}' not found"),
            )
        })?;
    if matches!(current.scope, VariableScope::Global) {
        let committed = state
            .update_global_variable_transaction(
                &project_instance_id,
                variable_id,
                name,
                data_type,
                data_value,
                description,
                tags,
                expected_revision,
                operation_id,
            )
            .map_err(AppError::from)?;
        emit_global_result(&mut emit, &committed.result);
        return Ok(VariableCommandResult {
            variable_id: committed.variable.id.to_string(),
            variable: Some((&committed.variable).into()),
            result: Some(committed.result),
        });
    }
    let updated = state
        .update_variable(&variable_id, name, data_type, data_value, description, tags)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "variable_not_found",
                format!("Variable '{variable_id}' not found"),
            )
        })?;
    emit(Event::Variable(EventVariable::VariableUpdated {
        variable_id: updated.id,
        variable_scope: updated.scope.clone(),
        data: (&updated).into(),
    }));
    Ok(VariableCommandResult {
        variable_id: updated.id.to_string(),
        variable: Some((&updated).into()),
        result: None,
    })
}

fn delete_variable_with_emitter(
    state: &ProjectState,
    variable_id: VariableId,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<VariableCommandResult, AppError> {
    ensure_command_project(state, &project_instance_id)?;
    let current = state
        .get_variable(&variable_id)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "variable_not_found",
                format!("Variable '{variable_id}' not found"),
            )
        })?;
    if matches!(current.scope, VariableScope::Global) {
        let committed = state
            .delete_global_variable_transaction(
                &project_instance_id,
                variable_id,
                expected_revision,
                operation_id,
            )
            .map_err(AppError::from)?;
        emit_global_result(&mut emit, &committed.result);
        return Ok(VariableCommandResult {
            variable_id: committed.variable.id.to_string(),
            variable: None,
            result: Some(committed.result),
        });
    }
    let removed = state
        .remove_variable(&variable_id)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "variable_not_found",
                format!("Variable '{variable_id}' not found"),
            )
        })?;
    emit(Event::Variable(EventVariable::VariableDeleted {
        variable_id: removed.id,
        variable_scope: removed.scope,
    }));
    Ok(VariableCommandResult {
        variable_id: removed.id.to_string(),
        variable: None,
        result: None,
    })
}

fn ensure_variable_data_type(data_type: &DataType) -> Result<(), AppError> {
    if matches!(data_type, DataType::Any) {
        return Err(AppError::new(
            "invalid_variable_type",
            "Variable data type cannot be Any",
        ));
    }
    Ok(())
}

/// 创建变量（统一接口，支持全局和局部变量）
#[tauri::command]
pub fn create_variable(
    state: State<ProjectState>,
    app: AppHandle,
    name: &str,
    data_type: DataType,
    data_value: DataValue,
    description: &str,
    scope: VariableScope,
    tags: Vec<String>,
    project_instance_id: ProjectInstanceId,
    expected_collection_revision: u64,
    operation_id: OperationId,
) -> Result<VariableCommandResult, AppError> {
    create_variable_with_emitter(
        state.inner(),
        name,
        data_type,
        data_value,
        description,
        scope,
        tags,
        project_instance_id,
        expected_collection_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

/// 获取变量（统一接口）
#[tauri::command]
pub fn get_variable(
    _app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
    project_instance_id: ProjectInstanceId,
) -> Result<VariableInstanceDTO, AppError> {
    ensure_command_project(state.inner(), &project_instance_id)?;
    let variable = state
        .get_variable(&variable_id)
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::new(
                "variable_not_found",
                format!("Variable '{}' not found", variable_id),
            )
        })?;
    Ok((&variable).into())
}

/// 更新变量（统一接口，部分更新）
#[tauri::command]
pub fn update_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
    name: Option<String>,
    data_type: Option<DataType>,
    data_value: Option<DataValue>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<VariableCommandResult, AppError> {
    update_variable_with_emitter(
        state.inner(),
        variable_id,
        name,
        data_type,
        data_value,
        description,
        tags,
        project_instance_id,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

/// 删除变量（统一接口）
#[tauri::command]
pub fn delete_variable(
    app: AppHandle,
    state: State<ProjectState>,
    variable_id: VariableId,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<VariableCommandResult, AppError> {
    delete_variable_with_emitter(
        state.inner(),
        variable_id,
        project_instance_id,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventProject};
    use crate::node_system::document::{
        HistoryMutation, MutationRequest, OperationId, ResourceKey, ResourceRevision,
        VariableResourceKey,
    };
    use crate::project::ProjectData;
    use std::collections::BTreeMap;

    fn active_state(label: &str) -> (std::path::PathBuf, ProjectState, ProjectInstanceId) {
        let root = std::env::temp_dir().join(format!(
            "yssbi-global-command-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        (root, state, project_instance_id)
    }

    fn command_snapshot(
        state: &ProjectState,
    ) -> (
        serde_json::Value,
        BTreeMap<ResourceKey, ResourceRevision>,
        u64,
        u64,
        serde_json::Value,
    ) {
        let session = state.capture_project_session().unwrap();
        let (_, publication_revision, history, _) =
            state.coherent_project_read_snapshot(&session).unwrap();
        (
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            state.global_variable_revision_snapshot(),
            state.authority_generation_for_test(),
            publication_revision,
            serde_json::to_value(history).unwrap(),
        )
    }

    #[test]
    fn create_global_variable_persistence_failure_has_zero_effects() {
        let (root, state, project_instance_id) = active_state("create-failure");
        let before = command_snapshot(&state);
        let mut events = Vec::new();
        crate::project::set_project_filesystem_fault(Some(
            crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
        ));

        let error = create_variable_with_emitter(
            &state,
            "global",
            DataType::Int64,
            DataValue::Int64(1),
            "",
            VariableScope::Global,
            vec![],
            project_instance_id,
            0,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        crate::project::set_project_filesystem_fault(None);

        assert_eq!(error.code, "transaction_prepare_failed", "{error:?}");
        assert_eq!(command_snapshot(&state), before);
        assert!(events.is_empty());
        assert!(!root.join(crate::project::GLOBAL_VARIABLES_FILE).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn update_global_variable_persistence_failure_has_zero_effects() {
        let (root, state, project_instance_id) = active_state("update-failure");
        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        state
            .persist_global_variables(
                &project_instance_id,
                state.global_variable_revision_snapshot(),
                OperationId::new(),
            )
            .unwrap();
        let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
        let before = command_snapshot(&state);
        let mut events = Vec::new();
        crate::project::set_project_filesystem_fault(Some(
            crate::project::ProjectFilesystemFaultPoint::FirstLiveReplacement,
        ));

        let error = update_variable_with_emitter(
            &state,
            variable.id,
            Some("changed".into()),
            None,
            Some(DataValue::Int64(2)),
            None,
            None,
            project_instance_id,
            ResourceRevision::INITIAL,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        crate::project::set_project_filesystem_fault(None);

        assert_eq!(error.code, "transaction_commit_failed");
        assert_eq!(command_snapshot(&state), before);
        assert_eq!(
            std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
            disk_before
        );
        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn delete_global_variable_persistence_failure_has_zero_effects() {
        let (root, state, project_instance_id) = active_state("delete-failure");
        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        state
            .persist_global_variables(
                &project_instance_id,
                state.global_variable_revision_snapshot(),
                OperationId::new(),
            )
            .unwrap();
        let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
        let before = command_snapshot(&state);
        let mut events = Vec::new();
        crate::project::set_project_filesystem_fault(Some(
            crate::project::ProjectFilesystemFaultPoint::FirstLiveReplacement,
        ));

        let error = delete_variable_with_emitter(
            &state,
            variable.id,
            project_instance_id,
            ResourceRevision::INITIAL,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        crate::project::set_project_filesystem_fault(None);

        assert_eq!(error.code, "transaction_commit_failed");
        assert_eq!(command_snapshot(&state), before);
        assert_eq!(
            std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
            disk_before
        );
        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn global_variable_commands_publish_contiguous_revisions_history_and_events() {
        let (root, state, project_instance_id) = active_state("success");
        let mut events = Vec::new();
        let created = create_variable_with_emitter(
            &state,
            "global",
            DataType::Int64,
            DataValue::Int64(1),
            "",
            VariableScope::Global,
            vec![],
            project_instance_id.clone(),
            0,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        let variable_id = *state.get_data().unwrap().variables.keys().next().unwrap();
        assert_eq!(created.result.as_ref().unwrap().publication_revision, 1);

        let updated = update_variable_with_emitter(
            &state,
            variable_id,
            None,
            None,
            Some(DataValue::Int64(2)),
            None,
            None,
            project_instance_id.clone(),
            ResourceRevision::new(1),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(updated.result.as_ref().unwrap().publication_revision, 2);
        assert!(updated.result.as_ref().unwrap().history.can_undo);
        assert_eq!(
            state
                .global_variable_revision_snapshot()
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![ResourceRevision::new(2)]
        );

        let deleted = delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id,
            ResourceRevision::new(2),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(deleted.result.as_ref().unwrap().publication_revision, 3);
        assert!(state.global_variable_revision_snapshot().is_empty());
        assert_eq!(events.len(), 3);
        for (event, expected_revision) in events.iter().zip(1_u64..=3) {
            assert!(matches!(
                event,
                Event::Project(EventProject::ResourceMutationCommitted { result })
                    if result.publication_revision == expected_revision
            ));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn global_persist_command_preserves_identity_operation_and_event_contract() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-global-command-contract-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let operation_id = OperationId::new();
        let key = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{}", variable.id).into(),
        ));
        let mut events = Vec::new();

        let result = persist_global_variables_with_emitter(
            &state,
            project_instance_id.clone(),
            BTreeMap::from([(key, ResourceRevision::INITIAL)]),
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

        state.activate_project_fixture(
            root.to_string_lossy().into_owned(),
            state.get_data().unwrap(),
        );
        let error = persist_global_variables_with_emitter(
            &state,
            project_instance_id,
            state.global_variable_revision_snapshot(),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(error.code, "stale_project_lifecycle");
        assert_eq!(events.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn global_commands_require_caller_basis_and_return_emit_one_canonical_result() {
        let (root, state, project_instance_id) = active_state("canonical-contract");
        let mut events = Vec::new();
        let created = create_variable_with_emitter(
            &state,
            "global",
            DataType::Int64,
            DataValue::Int64(1),
            "created",
            VariableScope::Global,
            vec!["initial".into()],
            project_instance_id.clone(),
            0,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        let created_result = created.result.as_ref().unwrap();
        assert_eq!(created_result.publication_revision, 1);
        assert!(matches!(
            &events[0],
            Event::Project(EventProject::ResourceMutationCommitted { result })
                if result == created_result
        ));

        let variable_id = *state.get_data().unwrap().variables.keys().next().unwrap();
        let updated = update_variable_with_emitter(
            &state,
            variable_id,
            Some("renamed".into()),
            Some(DataType::Float64),
            Some(DataValue::Float64(2.5)),
            Some("updated".into()),
            Some(vec!["changed".into()]),
            project_instance_id.clone(),
            ResourceRevision::new(1),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        let updated_result = updated.result.as_ref().unwrap();
        assert_eq!(updated_result.publication_revision, 2);
        let patch = match &updated_result.deltas[0].payload {
            crate::node_system::document::ResourceDocumentPatch::Variable(patch) => patch,
            payload => panic!("unexpected variable payload: {payload:?}"),
        };
        assert_eq!(patch.before.as_ref().unwrap()["name"], "global");
        assert_eq!(patch.after.as_ref().unwrap()["name"], "renamed");

        let stale = delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id.clone(),
            ResourceRevision::INITIAL,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(stale.code, "resource_revision_conflict");
        assert_eq!(events.len(), 2);

        let deleted = delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id,
            ResourceRevision::new(2),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(deleted.result.as_ref().unwrap().publication_revision, 3);
        assert_eq!(events.len(), 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn global_create_update_delete_history_restores_full_documents_and_publishes_once() {
        let (root, state, project_instance_id) = active_state("crud-history");
        let created = create_variable_with_emitter(
            &state,
            "before",
            DataType::Int64,
            DataValue::Int64(1),
            "created",
            VariableScope::Global,
            vec!["create".into()],
            project_instance_id.clone(),
            0,
            OperationId::new(),
            |_| {},
        )
        .unwrap();
        let variable_id = *state.get_data().unwrap().variables.keys().next().unwrap();
        update_variable_with_emitter(
            &state,
            variable_id,
            Some("after".into()),
            Some(DataType::Float64),
            Some(DataValue::Float64(2.5)),
            Some("updated".into()),
            Some(vec!["update".into()]),
            project_instance_id.clone(),
            ResourceRevision::new(1),
            OperationId::new(),
            |_| {},
        )
        .unwrap();
        delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id,
            ResourceRevision::new(2),
            OperationId::new(),
            |_| {},
        )
        .unwrap();
        let resource = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{variable_id}").into(),
        ));
        let mut publications = Vec::new();

        let undo_delete = state
            .undo_last_transaction_observed(
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    ResourceRevision::new(3),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |result| publications.push(result.clone()),
            )
            .unwrap();
        assert_eq!(undo_delete.publication_revision, 4);
        let restored = state.get_variable(&variable_id).unwrap().unwrap();
        assert_eq!(restored.name, "after");
        assert_eq!(restored.description, "updated");
        assert_eq!(restored.tags, vec!["update"]);

        let undo_update = state
            .undo_last_transaction_observed(
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    ResourceRevision::new(4),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |result| publications.push(result.clone()),
            )
            .unwrap();
        assert_eq!(undo_update.publication_revision, 5);
        let original = state.get_variable(&variable_id).unwrap().unwrap();
        assert_eq!(original.name, "before");
        assert_eq!(original.description, "created");
        assert_eq!(original.tags, vec!["create"]);

        state
            .undo_last_transaction_observed(
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    ResourceRevision::new(5),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |result| publications.push(result.clone()),
            )
            .unwrap();
        assert!(state.get_variable(&variable_id).unwrap().is_none());

        for revision in 6..=8 {
            state
                .redo_last_transaction_observed(
                    "en-US",
                    MutationRequest::new(
                        resource.clone(),
                        ResourceRevision::new(revision),
                        OperationId::new(),
                        HistoryMutation {},
                    ),
                    |result| publications.push(result.clone()),
                )
                .unwrap();
        }
        assert!(state.get_variable(&variable_id).unwrap().is_none());
        assert_eq!(publications.len(), 6);
        assert_eq!(
            publications
                .iter()
                .map(|result| result.publication_revision)
                .collect::<Vec<_>>(),
            vec![4, 5, 6, 7, 8, 9]
        );
        assert_eq!(created.result.unwrap().publication_revision, 1);
        let reloaded =
            crate::project::load_project_from_file(root.to_string_lossy().as_ref()).unwrap();
        assert!(!reloaded.variables.contains_key(&variable_id));
        let _ = std::fs::remove_dir_all(root);
    }
}
