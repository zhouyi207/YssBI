use super::*;

fn tabular_variable(
    name: &str,
    scope: crate::variable::VariableScope,
    values: &str,
) -> crate::variable::VariableInstance {
    let id = crate::variable::VariableId::new();
    let mut variable = crate::variable::VariableInstance {
        id,
        name: name.into(),
        data_type: crate::data_contract::DataType::DataFrame,
        data_value: crate::data_contract::DataValue::DataFrame(values.into()),
        tabular: None,
        description: String::new(),
        scope,
        tags: Vec::new(),
    };
    crate::tabular::normalize_variable_tabular(&mut variable).unwrap();
    variable
}

fn authoritative_tabular_json(
    state: &ProjectState,
    variable_id: crate::variable::VariableId,
) -> String {
    state
        .get_variable(&variable_id)
        .unwrap()
        .unwrap()
        .tabular
        .unwrap()
        .to_json()
        .unwrap()
}

fn commit_tabular_effect(
    state: &ProjectState,
    variable: &crate::variable::VariableInstance,
    values: &str,
) -> Result<VariableEffectCommitResult, VariableEffectCommitError> {
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    state.commit_variable_effects(
        &session_id,
        vec![crate::node_system::runtime::VariableWriteEffect {
            resource: crate::node_system::plan::ResourceId::new(format!(
                "variables/{}",
                variable.id
            ))
            .unwrap(),
            expected_revision: ResourceRevision::INITIAL,
            before: variable.clone(),
            after: crate::data_contract::DataValue::DataFrame(values.into()),
        }],
    )
}

#[test]
fn global_variable_effect_undo_redo_remains_equal_to_reloaded_disk() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-global-variable-effect-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
        format!("variables/{}", variable.id).into(),
    ));
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: ResourceRevision::INITIAL,
                before: variable.clone(),
                after: crate::data_contract::DataValue::Int64(2),
            }],
        )
        .unwrap();

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::project::load_project_from_file(&root_text)
            .unwrap()
            .variables[&variable.id]
            .data_value
    );

    state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                ResourceRevision::from_graph_revision(GraphRevision::new(2)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    let canonical = state.get_variable(&variable.id).unwrap().unwrap();
    let reloaded = crate::project::load_project_from_file(&root_text).unwrap();
    assert_eq!(
        serde_json::to_value(&canonical).unwrap(),
        serde_json::to_value(&reloaded.variables[&variable.id]).unwrap()
    );
    assert_eq!(
        canonical.data_value,
        crate::data_contract::DataValue::Int64(2)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_variable_effect_undo_redo_remains_equal_to_reloaded_disk() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-local-variable-effect-history-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let graph_path = GraphResourcePath::new("events/Local.yssbi-event").unwrap();
    let mut variable = test_variable("Local Rate");
    variable.scope = crate::variable::VariableScope::Event {
        event_path: graph_path.as_str().into(),
    };
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Local", GraphDocumentKind::Event),
    );
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
        format!("variables/{}", variable.id).into(),
    ));
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: ResourceRevision::INITIAL,
                before: variable.clone(),
                after: crate::data_contract::DataValue::Int64(2),
            }],
        )
        .unwrap();
    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource.clone(),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    state
        .redo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                resource,
                ResourceRevision::from_graph_revision(GraphRevision::new(2)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();

    let canonical = state.get_variable(&variable.id).unwrap().unwrap();
    let reloaded: crate::project::project_io::GraphDocument =
        serde_json::from_slice(&std::fs::read(root.join(graph_path.as_str())).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&canonical).unwrap(),
        serde_json::to_value(&reloaded.local_variables[&variable.id]).unwrap()
    );
    assert_eq!(
        canonical.data_value,
        crate::data_contract::DataValue::Int64(2)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn durable_variable_history_conflict_rolls_disk_back_without_authority_transfer() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-history-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: ResourceRevision::INITIAL,
                before: variable.clone(),
                after: crate::data_contract::DataValue::Int64(2),
            }],
        )
        .unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
    let conflict_state = state.clone();
    state.set_mutation_publication_test_hook(std::sync::Arc::new(move || {
        conflict_state.append_history_head_for_test();
    }));

    let error = state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                    format!("variables/{}", variable.id).into(),
                )),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap_err();

    assert!(matches!(error, MutationConflict::History(_)));
    assert_eq!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::data_contract::DataValue::Int64(2)
    );
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );
    assert!(state.history_status().can_undo);
    assert!(!state.history_status().can_redo);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn tabular_variable_effect_updates_global_and_local_authority() {
    for (label, scope, graph_path) in [
        ("global", crate::variable::VariableScope::Global, None),
        (
            "local",
            crate::variable::VariableScope::Event {
                event_path: "events/Tabular.yssbi-event".into(),
            },
            Some(GraphResourcePath::new("events/Tabular.yssbi-event").unwrap()),
        ),
    ] {
        let root = std::env::temp_dir().join(format!(
            "yssbi-{label}-tabular-variable-effect-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root_text = root.to_string_lossy().into_owned();
        let variable = tabular_variable("Table", scope, r#"{"value":[1,2]}"#);
        let mut project = ProjectData::new();
        if let Some(path) = &graph_path {
            project.graphs.insert(
                path.clone(),
                GraphResourceDocument::new("Tabular", GraphDocumentKind::Event),
            );
        }
        project.variables.insert(variable.id, variable.clone());
        crate::project::fixtures::write_project(&project, &root_text).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root_text, project);
        assert_eq!(
            authoritative_tabular_json(&state, variable.id),
            r#"{"value":[1,2]}"#
        );

        commit_tabular_effect(&state, &variable, r#"{"value":[7,8,9]}"#).unwrap();

        let canonical = state.get_variable(&variable.id).unwrap().unwrap();
        assert_eq!(
            canonical.data_value,
            crate::data_contract::DataValue::DataFrame(crate::tabular::variable_handle(
                &variable.id
            ))
        );
        assert_eq!(
            canonical.tabular.unwrap().to_json().unwrap(),
            r#"{"value":[7,8,9]}"#
        );
        assert_eq!(
            state.variable_revisions.read().unwrap()[&variable.id].revision,
            ResourceRevision::from_graph_revision(GraphRevision::new(1))
        );
        let resource = ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
            format!("variables/{}", variable.id).into(),
        ));
        state
            .undo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    resource.clone(),
                    ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_eq!(
            authoritative_tabular_json(&state, variable.id),
            r#"{"value":[1,2]}"#
        );
        state
            .redo_last_transaction_observed(
                &current_project_instance_id(&state),
                "en-US",
                MutationRequest::new(
                    resource,
                    ResourceRevision::from_graph_revision(GraphRevision::new(2)),
                    OperationId::new(),
                    HistoryMutation {},
                ),
                |_| {},
            )
            .unwrap();
        assert_eq!(
            authoritative_tabular_json(&state, variable.id),
            r#"{"value":[7,8,9]}"#
        );
        let disk_variable = if let Some(path) = &graph_path {
            let document: crate::project::project_io::GraphDocument =
                serde_json::from_slice(&std::fs::read(root.join(path.as_str())).unwrap()).unwrap();
            document.local_variables[&variable.id].clone()
        } else {
            crate::project::load_project_from_file(root.to_string_lossy().as_ref())
                .unwrap()
                .variables[&variable.id]
                .clone()
        };
        assert_eq!(
            serde_json::to_value(state.get_variable(&variable.id).unwrap().unwrap()).unwrap(),
            serde_json::to_value(disk_variable).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn failed_tabular_variable_effect_changes_neither_authority_nor_disk() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-failed-tabular-variable-effect-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = tabular_variable(
        "Local Table",
        crate::variable::VariableScope::Event {
            event_path: "events/Tabular.yssbi-event".into(),
        },
        r#"{"value":[1,2]}"#,
    );
    let graph_path = GraphResourcePath::new("events/Tabular.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Tabular", GraphDocumentKind::Event),
    );
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(graph_path.as_str())).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let authority_before = state.get_variable(&variable.id).unwrap().unwrap();

    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
    ));
    assert!(commit_tabular_effect(&state, &variable, r#"{"value":[7,8,9]}"#).is_err());

    assert_eq!(
        serde_json::to_value(state.get_variable(&variable.id).unwrap().unwrap()).unwrap(),
        serde_json::to_value(authority_before).unwrap()
    );
    assert_eq!(
        std::fs::read(root.join(graph_path.as_str())).unwrap(),
        disk_before
    );
    assert_eq!(
        state.variable_revisions.read().unwrap()[&variable.id].revision,
        ResourceRevision::from_graph_revision(GraphRevision::INITIAL)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn variable_effect_commit_is_revisioned_and_undoable() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-commit-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let committed = state
        .commit_variable_effects(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource,
                expected_revision: ResourceRevision::INITIAL,
                before: variable.clone(),
                after: crate::data_contract::DataValue::Int64(2),
            }],
        )
        .unwrap();
    assert_eq!(committed.variable_ids.as_ref(), &[variable.id]);
    let event_result = committed.resource_mutation.clone().unwrap();
    assert_eq!(event_result.publication_revision, 1);
    assert_eq!(event_result.deltas.len(), 1);
    assert_eq!(
        event_result.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::INITIAL)
    );
    assert_eq!(
        event_result.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(1))
    );
    assert_eq!(
        event_result.history,
        crate::node_system::document::HistoryStatusDto {
            can_undo: true,
            can_redo: false,
        }
    );
    assert!(event_result.projection_replacements.is_empty());
    assert_eq!(
        event_result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: Vec::new(),
        }
    );

    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::data_contract::DataValue::Int64(2)
    ));

    state
        .undo_last_transaction_observed(
            &current_project_instance_id(&state),
            "en-US",
            MutationRequest::new(
                ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                    format!("variables/{}", variable.id).into(),
                )),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                HistoryMutation {},
            ),
            |_| {},
        )
        .unwrap();
    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::data_contract::DataValue::Int64(1)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn variable_effect_persistence_failure_rolls_back_before_publication() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-transaction-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();

    let state = ProjectState::new();
    state.activate_project_fixture(root_text.clone(), project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let effect = crate::node_system::runtime::VariableWriteEffect {
        resource,
        expected_revision: ResourceRevision::INITIAL,
        before: variable.clone(),
        after: crate::data_contract::DataValue::Int64(2),
    };
    let history_before = state.history_status();
    let active_project_instance_id = state.capture_project_session().unwrap().instance_id;

    state.set_project_filesystem_fault(Some(
        crate::project::ProjectFilesystemFaultPoint::StagedSerialization,
    ));
    let error = state
        .commit_variable_effects(&session_id, vec![effect.clone()])
        .unwrap_err();

    assert!(matches!(
        error,
        VariableEffectCommitError::Persistence { .. }
    ));
    assert_eq!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::data_contract::DataValue::Int64(1)
    );
    assert_eq!(state.history_status(), history_before);
    let failed_index = state
        .read_project_index(&active_project_instance_id)
        .unwrap();
    assert_eq!(failed_index.publication_revision, 0);
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );

    let committed = state
        .commit_variable_effects(&session_id, vec![effect])
        .unwrap();
    let resource_mutation = committed.resource_mutation.as_ref().unwrap();
    assert_eq!(resource_mutation.publication_revision, 1);
    assert_eq!(
        resource_mutation.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::INITIAL)
    );
    assert_eq!(
        resource_mutation.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(1))
    );
    let success_index = state
        .read_project_index(&active_project_instance_id)
        .unwrap();
    assert_eq!(success_index.publication_revision, 1);
    assert_eq!(
        crate::project::load_project_from_file(&root_text)
            .unwrap()
            .variables[&variable.id]
            .data_value,
        crate::data_contract::DataValue::Int64(2)
    );

    let function_path = GraphResourcePath::new("functions/Next.yssbi-function").unwrap();
    state
        .insert_graph(
            function_path.clone(),
            GraphResourceDocument::new("Next", GraphDocumentKind::Function),
        )
        .unwrap();
    let next = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    function_path.as_str().into(),
                )),
                ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                OperationId::new(),
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    crate::node_system::document::FunctionSignature {
                        parameters: Vec::new(),
                        return_type: Some("Int64".into()),
                    },
                ),
            ),
            |_| {},
        )
        .unwrap();
    assert_eq!(next.publication_revision, 2);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn variable_effect_authority_assignment_panic_restores_every_authoritative_projection() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-authority-panic-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Panic Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let disk_before = std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let data_before = serde_json::to_value(state.get_data().unwrap()).unwrap();
    let history_before = state.history_status();
    let history_lengths_before = state.history_lengths_for_test();
    let revisions_before = state.revision_state_for_test();
    let publication_before = state.publication_state_for_test();

    let assignment_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let assignment_count_for_hook = std::sync::Arc::clone(&assignment_count);
    state.set_variable_authority_assignment_panic_for_test(std::sync::Arc::new(move || {
        if assignment_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
            panic!("injected variable authority assignment panic")
        }
    }));

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = state.commit_variable_effects_for_run(
            &session_id,
            vec![crate::node_system::runtime::VariableWriteEffect {
                resource: crate::node_system::plan::ResourceId::new(format!(
                    "variables/{}",
                    variable.id
                ))
                .unwrap(),
                expected_revision: ResourceRevision::INITIAL,
                before: variable.clone(),
                after: crate::data_contract::DataValue::Int64(2),
            }],
            &crate::node_system::runtime::CancellationToken::new(),
            None,
        );
    }));

    assert!(panic.is_err());
    assert_eq!(
        assignment_count.load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    assert_eq!(
        serde_json::to_value(state.get_data().unwrap()).unwrap(),
        data_before
    );
    assert_eq!(state.history_status(), history_before);
    assert_eq!(state.history_lengths_for_test(), history_lengths_before);
    assert_eq!(state.revision_state_for_test(), revisions_before);
    assert_eq!(state.publication_state_for_test(), publication_before);
    assert_eq!(
        std::fs::read(root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        disk_before
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_variable_effect_commit_returns_structured_revision_conflict() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-variable-effect-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root_text = root.to_string_lossy().into_owned();
    let variable = test_variable("Rate");
    let mut project = ProjectData::new();
    project.variables.insert(variable.id, variable.clone());
    crate::project::fixtures::write_project(&project, &root_text).unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root_text, project);
    let session_id = state
        .project_store
        .read()
        .unwrap()
        .project_session_id
        .clone();
    let resource =
        crate::node_system::plan::ResourceId::new(format!("variables/{}", variable.id)).unwrap();
    let stale_effect = crate::node_system::runtime::VariableWriteEffect {
        resource,
        expected_revision: ResourceRevision::INITIAL,
        before: variable.clone(),
        after: crate::data_contract::DataValue::Int64(2),
    };
    let winning_effect = crate::node_system::runtime::VariableWriteEffect {
        after: crate::data_contract::DataValue::Int64(3),
        ..stale_effect.clone()
    };
    state
        .commit_variable_effects(&session_id, vec![winning_effect])
        .unwrap();

    let error = state
        .commit_variable_effects(&session_id, vec![stale_effect])
        .unwrap_err();
    assert!(matches!(
        error,
        VariableEffectCommitError::Conflict {
            resource: ResourceKey::Variable(_),
            ..
        }
    ));
    assert!(matches!(
        state
            .get_variable(&variable.id)
            .unwrap()
            .unwrap()
            .data_value,
        crate::data_contract::DataValue::Int64(3)
    ));
    std::fs::remove_dir_all(root).unwrap();
}
