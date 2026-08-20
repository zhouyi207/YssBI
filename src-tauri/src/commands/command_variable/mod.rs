use crate::error::CommandError;
use crate::event::ResourceMutationResultDto;
use crate::event::{Event, EventProject, emit_project_event};
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
) -> Result<(), CommandError> {
    let session = state
        .capture_project_session()
        .map_err(CommandError::from)?;
    if &session.instance_id != project_instance_id {
        return Err(CommandError::expected("stale_project_lifecycle"));
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
) -> Result<ProjectSaveResultDto, CommandError> {
    let result = state
        .persist_global_variables(&project_instance_id, expected_revisions, operation_id)
        .map_err(CommandError::from)?;
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
) -> Result<VariableCommandResult, CommandError> {
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
            .map_err(CommandError::from)?;
        emit_global_result(&mut emit, &committed.result);
        return Ok(VariableCommandResult {
            variable_id: committed.variable.id.to_string(),
            variable: Some((&committed.variable).into()),
            result: Some(committed.result),
        });
    }
    let committed = state
        .create_local_variable_transaction(
            &project_instance_id,
            name.to_string(),
            data_type,
            data_value,
            description.to_string(),
            scope,
            tags,
            expected_collection_revision,
            operation_id,
        )
        .map_err(CommandError::from)?;
    emit_global_result(&mut emit, &committed.result);
    Ok(VariableCommandResult {
        variable_id: committed.variable.id.to_string(),
        variable: Some((&committed.variable).into()),
        result: Some(committed.result),
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
) -> Result<VariableCommandResult, CommandError> {
    ensure_command_project(state, &project_instance_id)?;
    if let Some(ref data_type) = data_type {
        ensure_variable_data_type(data_type)?;
    }
    let current = state
        .get_variable(&variable_id)
        .map_err(CommandError::from)?
        .ok_or_else(|| CommandError::expected("variable_not_found"))?;
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
            .map_err(CommandError::from)?;
        emit_global_result(&mut emit, &committed.result);
        return Ok(VariableCommandResult {
            variable_id: committed.variable.id.to_string(),
            variable: Some((&committed.variable).into()),
            result: Some(committed.result),
        });
    }
    let committed = state
        .update_local_variable_transaction(
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
        .map_err(CommandError::from)?;
    emit_global_result(&mut emit, &committed.result);
    Ok(VariableCommandResult {
        variable_id: committed.variable.id.to_string(),
        variable: Some((&committed.variable).into()),
        result: Some(committed.result),
    })
}

fn delete_variable_with_emitter(
    state: &ProjectState,
    variable_id: VariableId,
    project_instance_id: ProjectInstanceId,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mut emit: impl FnMut(Event),
) -> Result<VariableCommandResult, CommandError> {
    ensure_command_project(state, &project_instance_id)?;
    let current = state
        .get_variable(&variable_id)
        .map_err(CommandError::from)?
        .ok_or_else(|| CommandError::expected("variable_not_found"))?;
    if matches!(current.scope, VariableScope::Global) {
        let committed = state
            .delete_global_variable_transaction(
                &project_instance_id,
                variable_id,
                expected_revision,
                operation_id,
            )
            .map_err(CommandError::from)?;
        emit_global_result(&mut emit, &committed.result);
        return Ok(VariableCommandResult {
            variable_id: committed.variable.id.to_string(),
            variable: None,
            result: Some(committed.result),
        });
    }
    let committed = state
        .delete_local_variable_transaction(
            &project_instance_id,
            variable_id,
            expected_revision,
            operation_id,
        )
        .map_err(CommandError::from)?;
    emit_global_result(&mut emit, &committed.result);
    Ok(VariableCommandResult {
        variable_id: committed.variable.id.to_string(),
        variable: None,
        result: Some(committed.result),
    })
}

fn ensure_variable_data_type(data_type: &DataType) -> Result<(), CommandError> {
    if matches!(data_type, DataType::Any) {
        return Err(CommandError::expected("invalid_variable_type"));
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
) -> Result<VariableCommandResult, CommandError> {
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
) -> Result<VariableInstanceDTO, CommandError> {
    ensure_command_project(state.inner(), &project_instance_id)?;
    let variable = state
        .get_variable(&variable_id)
        .map_err(CommandError::from)?
        .ok_or_else(|| CommandError::expected("variable_not_found"))?;
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
) -> Result<VariableCommandResult, CommandError> {
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
) -> Result<VariableCommandResult, CommandError> {
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
        state.set_project_filesystem_fault(Some(
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
        state.set_project_filesystem_fault(None);

        assert_eq!(error.code(), "transaction_prepare_failed", "{error:?}");
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
        state.set_project_filesystem_fault(Some(
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
        state.set_project_filesystem_fault(None);

        assert_eq!(error.code(), "transaction_commit_failed");
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
        state.set_project_filesystem_fault(Some(
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
        state.set_project_filesystem_fault(None);

        assert_eq!(error.code(), "transaction_commit_failed");
        assert_eq!(command_snapshot(&state), before);
        assert_eq!(
            std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
            disk_before
        );
        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn update_tabular_variable_rejects_invalid_value_without_reusing_snapshot() {
        let (root, state, project_instance_id) = active_state("invalid-tabular-update");
        let created = create_variable_with_emitter(
            &state,
            "table",
            DataType::DataFrame,
            DataValue::DataFrame(r#"{"value":[1,2]}"#.into()),
            "",
            VariableScope::Global,
            vec![],
            project_instance_id.clone(),
            0,
            OperationId::new(),
            |_| {},
        )
        .unwrap();
        let before = command_snapshot(&state);
        let mut events = Vec::new();

        let error = update_variable_with_emitter(
            &state,
            VariableId::from(
                uuid::Uuid::parse_str(&created.variable_id).expect("created variable UUID"),
            ),
            None,
            None,
            Some(DataValue::DataFrame("not-json".into())),
            None,
            None,
            project_instance_id,
            ResourceRevision::new(1),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();

        assert_eq!(error.code(), "transaction_commit_failed");
        assert_eq!(command_snapshot(&state), before);
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
        assert_eq!(error.code(), "stale_project_lifecycle");
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
        assert_eq!(stale.code(), "resource_revision_conflict");
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
    fn local_commands_publish_once_validate_revisions_and_reject_duplicate_operations() {
        let (root, state, project_instance_id) = active_state("local-canonical-contract");
        let scope = VariableScope::Function {
            function_path: "functions/Local.yssbi-function".into(),
        };
        let create_operation = OperationId::new();
        let mut events = Vec::new();

        let stale_create = create_variable_with_emitter(
            &state,
            "stale",
            DataType::Int64,
            DataValue::Int64(0),
            "",
            scope.clone(),
            vec![],
            project_instance_id.clone(),
            9,
            create_operation,
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(stale_create.code(), "resource_revision_conflict");
        assert!(state.get_data().unwrap().variables.is_empty());
        assert!(events.is_empty());

        let created = create_variable_with_emitter(
            &state,
            "local",
            DataType::Int64,
            DataValue::Int64(1),
            "",
            scope.clone(),
            vec![],
            project_instance_id.clone(),
            0,
            create_operation,
            |event| events.push(event),
        )
        .unwrap();
        let variable_id = *state.get_data().unwrap().variables.keys().next().unwrap();
        assert_eq!(created.result.as_ref().unwrap().publication_revision, 1);
        assert_eq!(
            state.revision_state_for_test().1.get(&variable_id),
            Some(&ResourceRevision::new(1))
        );

        let duplicate = create_variable_with_emitter(
            &state,
            "duplicate",
            DataType::Int64,
            DataValue::Int64(2),
            "",
            scope,
            vec![],
            project_instance_id.clone(),
            1,
            create_operation,
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(duplicate.code(), "duplicate_operation");

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

        let delete_operation = OperationId::new();
        let stale = delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id.clone(),
            ResourceRevision::new(1),
            delete_operation,
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(stale.code(), "resource_revision_conflict");
        assert!(state.get_variable(&variable_id).unwrap().is_some());
        assert_eq!(events.len(), 2);

        let deleted = delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id,
            ResourceRevision::new(2),
            delete_operation,
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(deleted.result.as_ref().unwrap().publication_revision, 3);
        assert_eq!(
            state.revision_state_for_test().1.get(&variable_id),
            Some(&ResourceRevision::new(3))
        );
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|event| matches!(
            event,
            Event::Project(EventProject::ResourceMutationCommitted { .. })
        )));
        let _ = std::fs::remove_dir_all(root);
    }

    fn local_history_state(
        label: &str,
        variable: Option<crate::variable::VariableInstance>,
    ) -> (
        std::path::PathBuf,
        ProjectState,
        ProjectInstanceId,
        crate::project::GraphResourcePath,
    ) {
        let root = std::env::temp_dir().join(format!(
            "yssbi-local-history-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        let graph_path =
            crate::project::GraphResourcePath::new("events/Local.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            graph_path.clone(),
            crate::project::GraphResourceDocument::new(
                "Local",
                crate::project::GraphDocumentKind::Event,
            ),
        );
        if let Some(variable) = variable {
            data.variables.insert(variable.id, variable);
        }
        crate::project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        (root, state, project_instance_id, graph_path)
    }

    fn disk_local_variables(
        root: &std::path::Path,
        graph_path: &crate::project::GraphResourcePath,
    ) -> std::collections::HashMap<VariableId, crate::variable::VariableInstance> {
        let document: crate::project::project_io::GraphDocument =
            serde_json::from_slice(&std::fs::read(root.join(graph_path.as_str())).unwrap())
                .unwrap();
        document.local_variables
    }

    #[test]
    fn local_create_history_rewrites_owning_graph_when_selected_snapshot_is_absent() {
        let (root, state, project_instance_id, graph_path) = local_history_state("create", None);
        let scope = VariableScope::Event {
            event_path: graph_path.as_str().into(),
        };
        let created = create_variable_with_emitter(
            &state,
            "created",
            DataType::Int64,
            DataValue::Int64(1),
            "",
            scope,
            vec![],
            project_instance_id.clone(),
            0,
            OperationId::new(),
            |_| {},
        )
        .unwrap();
        let variable_id = *state.get_data().unwrap().variables.keys().next().unwrap();
        let resource = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{variable_id}").into(),
        ));
        assert_eq!(created.result.unwrap().publication_revision, 1);

        for (undo, base_revision, publication_revision) in
            [(true, 1, 2), (false, 2, 3), (true, 3, 4)]
        {
            let request = MutationRequest::new(
                resource.clone(),
                ResourceRevision::new(base_revision),
                OperationId::new(),
                HistoryMutation {},
            );
            let result = if undo {
                state.undo_last_transaction_observed(&project_instance_id, "en-US", request, |_| {})
            } else {
                state.redo_last_transaction_observed(&project_instance_id, "en-US", request, |_| {})
            }
            .unwrap();
            assert_eq!(result.publication_revision, publication_revision);
        }

        assert!(state.get_variable(&variable_id).unwrap().is_none());
        assert!(!disk_local_variables(&root, &graph_path).contains_key(&variable_id));
        state.unload_graph_resource(&graph_path).unwrap();
        state
            .load_graph_resource(&project_instance_id, &graph_path, 1)
            .unwrap();
        assert!(state.get_variable(&variable_id).unwrap().is_none());
        let entry = state
            .variable_revision_entry_for_test(&variable_id)
            .unwrap();
        assert_eq!(entry.revision, ResourceRevision::new(4));
        assert!(!entry.is_present());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn local_update_delete_history_persists_graph_and_preserves_publication_continuity() {
        let variable_id = VariableId::new();
        let variable = crate::variable::VariableInstance {
            id: variable_id,
            name: "before".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Event {
                event_path: "events/Local.yssbi-event".into(),
            },
            tags: Vec::new(),
        };
        let (root, state, project_instance_id, graph_path) =
            local_history_state("update-delete", Some(variable));
        let resource = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{variable_id}").into(),
        ));
        let mut events = Vec::new();

        let updated = update_variable_with_emitter(
            &state,
            variable_id,
            Some("after".into()),
            None,
            Some(DataValue::Int64(2)),
            None,
            None,
            project_instance_id.clone(),
            ResourceRevision::INITIAL,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(updated.result.unwrap().publication_revision, 1);

        for (undo, base_revision, publication_revision, expected_name) in
            [(true, 1, 2, "before"), (false, 2, 3, "after")]
        {
            let request = MutationRequest::new(
                resource.clone(),
                ResourceRevision::new(base_revision),
                OperationId::new(),
                HistoryMutation {},
            );
            let result = if undo {
                state.undo_last_transaction_observed(&project_instance_id, "en-US", request, |_| {})
            } else {
                state.redo_last_transaction_observed(&project_instance_id, "en-US", request, |_| {})
            }
            .unwrap();
            assert_eq!(result.publication_revision, publication_revision);
            assert_eq!(
                disk_local_variables(&root, &graph_path)[&variable_id].name,
                expected_name
            );
        }

        let deleted = delete_variable_with_emitter(
            &state,
            variable_id,
            project_instance_id.clone(),
            ResourceRevision::new(3),
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(deleted.result.unwrap().publication_revision, 4);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| matches!(
            event,
            Event::Project(EventProject::ResourceMutationCommitted { .. })
        )));

        let restored = state
            .undo_last_transaction_observed(
                &project_instance_id,
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    ResourceRevision::new(4),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_eq!(restored.publication_revision, 5);
        let restored_entry = state
            .variable_revision_entry_for_test(&variable_id)
            .unwrap();
        assert_eq!(restored_entry.revision, ResourceRevision::new(5));
        assert!(restored_entry.is_present());
        assert!(disk_local_variables(&root, &graph_path).contains_key(&variable_id));

        let removed = state
            .redo_last_transaction_observed(
                &project_instance_id,
                "en-US",
                MutationRequest::new(
                    resource,
                    ResourceRevision::new(5),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_eq!(removed.publication_revision, 6);
        assert!(!disk_local_variables(&root, &graph_path).contains_key(&variable_id));
        assert!(state.get_variable(&variable_id).unwrap().is_none());
        assert!(state.history_status().can_undo);

        state.unload_graph_resource(&graph_path).unwrap();
        state
            .load_graph_resource(&project_instance_id, &graph_path, 1)
            .unwrap();
        assert!(state.get_variable(&variable_id).unwrap().is_none());
        let entry = state
            .variable_revision_entry_for_test(&variable_id)
            .unwrap();
        assert_eq!(entry.revision, ResourceRevision::new(6));
        assert!(!entry.is_present());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn global_create_update_delete_history_restores_full_documents_and_publishes_once() {
        let (root, state, project_instance_id) = active_state("crud-history");
        crate::project::fixtures::write_project(
            &ProjectData::new(),
            root.to_string_lossy().as_ref(),
        )
        .unwrap();
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
            project_instance_id.clone(),
            ResourceRevision::new(2),
            OperationId::new(),
            |_| {},
        )
        .unwrap();
        assert!(state.get_variable(&variable_id).unwrap().is_none());
        assert_eq!(
            state.revision_state_for_test().1.get(&variable_id),
            Some(&ResourceRevision::new(3))
        );
        let resource = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{variable_id}").into(),
        ));
        let mut publications = Vec::new();

        let stale = state
            .undo_last_transaction_observed(
                &project_instance_id,
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    ResourceRevision::new(2),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |result| publications.push(result.clone()),
            )
            .unwrap_err();
        assert!(matches!(
            stale,
            crate::node_system::document::MutationConflict::StaleRevision {
                base_revision,
                current_revision,
            } if base_revision == ResourceRevision::new(2)
                && current_revision == ResourceRevision::new(3)
        ));
        assert!(publications.is_empty());

        let undo_delete = state
            .undo_last_transaction_observed(
                &project_instance_id,
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
                &project_instance_id,
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
                &project_instance_id,
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
        assert_eq!(
            state.revision_state_for_test().1.get(&variable_id),
            Some(&ResourceRevision::new(6))
        );

        for revision in 6..=8 {
            state
                .redo_last_transaction_observed(
                    &project_instance_id,
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
        assert_eq!(
            state.revision_state_for_test().1.get(&variable_id),
            Some(&ResourceRevision::new(9))
        );
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
