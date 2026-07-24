use super::*;
use crate::node_system::document::{
    DocumentNode, GraphDocumentOperation, GraphDocumentPatch, GraphRevision, HistoryMutation,
    MutationConflict, MutationRequest, OperationId, ParameterValues, ResourceKey,
};
use crate::node_system::protocol::NodeTypeId;
use crate::node_system::runtime::NOOP_RUN_EVENT_SINK;

fn graph_path() -> GraphResourcePath {
    GraphResourcePath::new("events/Production.yssbi-event").unwrap()
}

fn document_path() -> crate::node_system::document::GraphResourcePath {
    crate::node_system::document::GraphResourcePath(graph_path().as_str().into())
}

fn node(node_type: &str) -> DocumentNode {
    DocumentNode {
        id: crate::node_system::document::NodeId::new(),
        node_type: NodeTypeId::new(node_type).unwrap(),
        position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
        parameters: ParameterValues::new(),
        user_label: None,
    }
}

fn state_with_empty_graph() -> ProjectState {
    let state = ProjectState::new();
    state.insert_graph(
        graph_path(),
        GraphResourceDocument::new("Production", GraphDocumentKind::Event),
    );
    state
}

#[test]
fn normalized_graph_lifecycle_routes_every_insert_through_project_state() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-lifecycle-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::save_project_to_file(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.set_path(Some(root.to_string_lossy().into_owned()));

    let created = state
        .create_graph_resource("Lifecycle", GraphDocumentKind::Event)
        .unwrap();
    assert!(!state.get_data().graphs.contains_key(&created));
    let loaded = state.load_graph_from_current_project(&created).unwrap();
    assert_eq!(loaded.name, "Lifecycle");
    state.save_graph_resource(&created).unwrap();
    state.unload_graph_resource(&created);

    let duplicated = state.duplicate_graph_resource(&created).unwrap();
    assert_ne!(duplicated, created);
    assert!(!state.get_data().graphs.contains_key(&duplicated));
    let renamed = state
        .rename_graph_resource(&duplicated, "Lifecycle Copy Renamed")
        .unwrap();
    assert_ne!(renamed, duplicated);
    state.remove_graph_resource(&created).unwrap();
    state.remove_graph_resource(&renamed).unwrap();

    let index = crate::project::read_project_index(root.to_string_lossy().as_ref()).unwrap();
    assert!(index.graphs.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_duplicate_rebinds_self_identity_and_loaded_rename_is_authoritative() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-resource-identity-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::save_project_to_file(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.set_path(Some(root.to_string_lossy().into_owned()));
    let caller = state
        .create_graph_resource("Caller", GraphDocumentKind::Event)
        .unwrap();
    let function = state
        .create_graph_resource("Callee", GraphDocumentKind::Function)
        .unwrap();
    state.load_graph_from_current_project(&caller).unwrap();
    state.load_graph_from_current_project(&function).unwrap();
    let local_variable_id = crate::variable::VariableId::new();
    state.project_data.write().unwrap().variables.insert(
        local_variable_id,
        crate::variable::VariableInstance {
            id: local_variable_id,
            name: "Local Rate".into(),
            data_type: crate::graph::value::DataType::Int64,
            data_value: crate::graph::value::DataValue::Int64(9),
            tabular: None,
            description: String::new(),
            scope: crate::variable::VariableScope::Function {
                function_path: function.as_str().into(),
            },
            tags: Vec::new(),
        },
    );

    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function.as_str()),
    );
    state
        .apply_graph_patch(
            &caller,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    caller.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: call }]),
            ),
        )
        .unwrap();
    let duplicated = state.duplicate_graph_resource(&function).unwrap();
    let duplicate = state.load_graph_from_current_project(&duplicated).unwrap();
    for shell in duplicate.document.nodes.values().filter(|node| {
        matches!(
            node.node_type.as_str(),
            "yssbi.project.function.entry" | "yssbi.project.function.return"
        )
    }) {
        assert_eq!(
            shell
                .parameters
                .iter()
                .find(|(key, _)| key.as_str() == "function")
                .and_then(|(_, value)| value.as_str()),
            Some(duplicated.as_str())
        );
    }

    let renamed = state
        .rename_graph_resource(&function, "Renamed Callee")
        .unwrap();
    let data = state.get_data();
    let loaded_caller = &data.graphs[&caller];
    assert!(loaded_caller.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(renamed.as_str()))
    }));
    assert!(!loaded_caller.document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(function.as_str()))
    }));
    assert_eq!(
        data.variables[&local_variable_id].scope,
        crate::variable::VariableScope::Function {
            function_path: renamed.as_str().into(),
        }
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_replacement_during_function_loading_cancels_before_old_resource_insert() {
    let old_root = std::env::temp_dir().join(format!(
        "yssbi-production-loading-old-{}",
        uuid::Uuid::new_v4()
    ));
    let new_root = std::env::temp_dir().join(format!(
        "yssbi-production-loading-new-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&old_root).unwrap();
    std::fs::create_dir_all(&new_root).unwrap();
    crate::project::save_project_to_file(&ProjectData::new(), old_root.to_string_lossy().as_ref())
        .unwrap();
    crate::project::save_project_to_file(&ProjectData::new(), new_root.to_string_lossy().as_ref())
        .unwrap();

    let state = ProjectState::new();
    state.set_path(Some(old_root.to_string_lossy().into_owned()));
    let event = state
        .create_graph_resource("Loading Caller", GraphDocumentKind::Event)
        .unwrap();
    let old_function = state
        .create_graph_resource("Loading Callee", GraphDocumentKind::Function)
        .unwrap();
    state.load_graph_from_current_project(&event).unwrap();

    let (loading_tx, loading_rx) = std::sync::mpsc::channel();
    state.set_function_load_checkpoint(std::sync::Arc::new(
        move |cancellation: &crate::node_system::runtime::CancellationToken| {
            loading_tx.send(()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !cancellation.is_cancelled() && std::time::Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(cancellation.is_cancelled());
        },
    ));

    let executing_state = state.clone();
    let execution =
        std::thread::spawn(move || executing_state.execute_graph(&event, &NOOP_RUN_EVENT_SINK));
    loading_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();

    let replacement_state = state.clone();
    let replacement_path = new_root.to_string_lossy().into_owned();
    let replacement = std::thread::spawn(move || {
        replacement_state.activate_loaded_project(replacement_path, ProjectData::new());
    });

    let error = execution.join().unwrap().unwrap_err();
    assert!(
        error.contains("cancel"),
        "unexpected execution error: {error}"
    );
    replacement.join().unwrap();
    assert!(!state.get_data().graphs.contains_key(&old_function));
    assert_eq!(
        state.get_path().as_deref(),
        Some(new_root.to_string_lossy().as_ref())
    );

    std::fs::remove_dir_all(old_root).unwrap();
    std::fs::remove_dir_all(new_root).unwrap();
}

#[test]
fn normalized_function_signature_update_is_undoable() {
    let state = ProjectState::new();
    let path = GraphResourcePath::new("functions/Tax.yssbi-function").unwrap();
    state.insert_graph(
        path.clone(),
        GraphResourceDocument::new("Tax", GraphDocumentKind::Function),
    );
    let signature = crate::node_system::document::FunctionSignature {
        parameters: vec![crate::node_system::document::FunctionParameter {
            id: crate::node_system::document::FunctionParameterId("amount".into()),
            name: "Amount".into(),
            type_name: "float64".into(),
        }],
        return_type: Some("float64".into()),
    };

    let operation_id = OperationId::new();
    let delta = state
        .update_function_signature(
            &path,
            MutationRequest::new(
                ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                    path.as_str().into(),
                )),
                crate::node_system::document::ResourceRevision::INITIAL,
                operation_id,
                crate::node_system::document::FunctionDocumentPatch::new(
                    Default::default(),
                    signature.clone(),
                ),
            ),
        )
        .unwrap();
    assert_eq!(delta.from_revision.get(), 0);
    assert_eq!(delta.to_revision.get(), 1);
    assert_eq!(delta.caused_by, Some(operation_id));
    assert_eq!(
        state.get_data().graphs[&path]
            .function
            .as_ref()
            .unwrap()
            .signature,
        signature
    );
    state
        .undo_last_transaction(MutationRequest::new(
            ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
                path.as_str().into(),
            )),
            GraphRevision::new(1),
            OperationId::new(),
            HistoryMutation {},
        ))
        .unwrap();
    assert_eq!(
        state.get_data().graphs[&path]
            .function
            .as_ref()
            .unwrap()
            .signature,
        crate::node_system::document::FunctionSignature::default()
    );
}

#[test]
fn revisioned_signature_undo_and_redo_reject_conflicts_and_return_deltas() {
    let state = ProjectState::new();
    let path = GraphResourcePath::new("functions/Revisioned.yssbi-function").unwrap();
    state.insert_graph(
        path.clone(),
        GraphResourceDocument::new("Revisioned", GraphDocumentKind::Function),
    );
    let resource = ResourceKey::Function(crate::node_system::document::FunctionResourceKey(
        path.as_str().into(),
    ));
    let signature = crate::node_system::document::FunctionSignature {
        parameters: vec![crate::node_system::document::FunctionParameter {
            id: crate::node_system::document::FunctionParameterId("value".into()),
            name: "Value".into(),
            type_name: "float64".into(),
        }],
        return_type: Some("float64".into()),
    };
    let patch = crate::node_system::document::FunctionDocumentPatch::new(
        Default::default(),
        signature.clone(),
    );
    let signature_operation = OperationId::new();
    let signature_delta = state
        .update_function_signature(
            &path,
            MutationRequest::new(
                resource.clone(),
                GraphRevision::INITIAL,
                signature_operation,
                patch.clone(),
            ),
        )
        .unwrap();
    assert_eq!(signature_delta.caused_by, Some(signature_operation));
    assert_eq!(signature_delta.from_revision, GraphRevision::INITIAL);
    assert_eq!(signature_delta.to_revision, GraphRevision::new(1));
    assert!(matches!(
        state.update_function_signature(
            &path,
            MutationRequest::new(
                resource.clone(),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        ),
        Err(MutationConflict::StaleRevision { .. })
    ));

    let stale_undo = MutationRequest::new(
        resource.clone(),
        GraphRevision::INITIAL,
        OperationId::new(),
        HistoryMutation {},
    );
    assert!(matches!(
        state.undo_last_transaction(stale_undo),
        Err(MutationConflict::StaleRevision { .. })
    ));
    let undo_operation = OperationId::new();
    let undo_deltas = state
        .undo_last_transaction(MutationRequest::new(
            resource.clone(),
            GraphRevision::new(1),
            undo_operation,
            HistoryMutation {},
        ))
        .unwrap();
    assert_eq!(undo_deltas.len(), 1);
    assert_eq!(undo_deltas[0].resource, resource);
    assert_eq!(undo_deltas[0].from_revision, GraphRevision::new(1));
    assert_eq!(undo_deltas[0].to_revision, GraphRevision::new(2));
    assert_eq!(undo_deltas[0].caused_by, Some(undo_operation));

    let stale_redo = MutationRequest::new(
        undo_deltas[0].resource.clone(),
        GraphRevision::new(1),
        OperationId::new(),
        HistoryMutation {},
    );
    assert!(matches!(
        state.redo_last_transaction(stale_redo),
        Err(MutationConflict::StaleRevision { .. })
    ));
    let redo_operation = OperationId::new();
    let redo_deltas = state
        .redo_last_transaction(MutationRequest::new(
            undo_deltas[0].resource.clone(),
            GraphRevision::new(2),
            redo_operation,
            HistoryMutation {},
        ))
        .unwrap();
    assert_eq!(redo_deltas.len(), 1);
    assert_eq!(redo_deltas[0].from_revision, GraphRevision::new(2));
    assert_eq!(redo_deltas[0].to_revision, GraphRevision::new(3));
    assert_eq!(redo_deltas[0].caused_by, Some(redo_operation));
    assert_eq!(
        state.get_data().graphs[&path]
            .function
            .as_ref()
            .unwrap()
            .signature,
        signature
    );
}

#[test]
fn project_mutation_rejects_stale_revision_and_records_undo_history() {
    let state = state_with_empty_graph();
    let inserted = node("yssbi.constant.int64");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted.clone(),
    }]);
    let request = MutationRequest::new(
        ResourceKey::Graph(document_path()),
        GraphRevision::INITIAL,
        OperationId::new(),
        patch,
    );

    let event = state
        .apply_graph_patch(&graph_path(), request.clone())
        .unwrap();
    assert_eq!(event.from_revision, GraphRevision::INITIAL);
    assert_eq!(event.to_revision, GraphRevision::new(1));
    assert!(matches!(
        state.apply_graph_patch(&graph_path(), request),
        Err(MutationConflict::StaleRevision { .. })
    ));

    state
        .undo_last_transaction(MutationRequest::new(
            ResourceKey::Graph(document_path()),
            GraphRevision::new(1),
            OperationId::new(),
            HistoryMutation {},
        ))
        .unwrap();
    let graph = state.get_data().graphs.remove(&graph_path()).unwrap();
    assert!(graph.document.nodes.is_empty());
    assert_eq!(graph.document.revision, GraphRevision::new(2));
}

#[test]
fn project_projection_hydrates_localized_editor_dto() {
    let state = state_with_empty_graph();
    let inserted = node("yssbi.constant.int64");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
        node: inserted.clone(),
    }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let projection = state.graph_projection(&graph_path(), "zh-CN").unwrap();
    assert_eq!(projection.graph_path.as_ref(), graph_path().as_str());
    assert_eq!(projection.source_revision, 1);
    assert_eq!(projection.nodes.len(), 1);
    assert_eq!(
        projection.nodes[0].node_id.as_ref(),
        inserted.id.to_string()
    );
    assert!(!projection.nodes[0].display.title.is_empty());
}

#[test]
fn project_execution_publishes_persisted_function_plans() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-functions-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::save_project_to_file(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.set_path(Some(root.to_string_lossy().into_owned()));
    let event = state
        .create_graph_resource("Main", GraphDocumentKind::Event)
        .unwrap();
    state.load_graph_from_current_project(&event).unwrap();
    let function = state
        .create_graph_resource("Helper", GraphDocumentKind::Function)
        .unwrap();
    let begin = state.get_data().graphs[&event]
        .document
        .nodes
        .values()
        .find(|node| node.node_type.as_str() == "yssbi.project.event.begin")
        .unwrap()
        .id;
    let mut call = node("yssbi.project.function.call");
    call.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("target").unwrap(),
        serde_json::json!(function.as_str()),
    );
    let connection_id = crate::node_system::document::ConnectionId::new();
    let connection = crate::node_system::document::DocumentConnection {
        id: connection_id,
        output: crate::node_system::document::PortAddress::declared(
            begin,
            crate::node_system::protocol::PortKey::new("then").unwrap(),
        ),
        input: crate::node_system::document::PortAddress::declared(
            call.id,
            crate::node_system::protocol::PortKey::new("enter").unwrap(),
        ),
        order: None,
    };
    state
        .apply_graph_patch(
            &event,
            MutationRequest::new(
                crate::node_system::document::ResourceKey::Graph(
                    crate::node_system::document::GraphResourcePath(event.as_str().into()),
                ),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![
                    GraphDocumentOperation::InsertNode { node: call },
                    GraphDocumentOperation::InsertConnection { connection },
                ]),
            ),
        )
        .unwrap();

    state.execute_graph(&event, &NOOP_RUN_EVENT_SINK).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_compiler_rejects_wrong_scope_and_duplicate_shell_nodes() {
    let state = state_with_empty_graph();
    let first = node("yssbi.project.function.entry");
    let second = node("yssbi.project.function.entry");
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode { node: first },
        GraphDocumentOperation::InsertNode { node: second },
    ]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let error = state
        .execute_graph(&graph_path(), &NOOP_RUN_EVENT_SINK)
        .unwrap_err();
    assert!(error.contains("compiler.node.scope_mismatch"));
    assert!(error.contains("compiler.node.managed_singleton"));
}

#[test]
fn production_relational_backend_executes_project_dataframe_source() {
    use crate::node_system::runtime::RelationalBackend;
    let dataframe = polars::df!("value" => [1_i64, 2, 3]).unwrap();
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(
        crate::node_system::runtime::ProjectResourceSnapshot::new(
            crate::node_system::analysis::ProjectSessionId::new("relational-project"),
            crate::node_system::analysis::ResourceVersionSet::new(),
        )
        .with_database(resource.clone(), std::sync::Arc::new(dataframe)),
    );
    let requirement = crate::node_system::plan::CompiledResourceRequirement {
        resource: resource.clone(),
        kind: crate::node_system::plan::ResourceKind::DatabaseConnection,
        access: crate::node_system::plan::ResourceAccess::Shared,
        optional: false,
    };
    let resources =
        crate::node_system::runtime::RunResourceSet::acquire(&[requirement], &provider).unwrap();
    let cancellation = crate::node_system::runtime::CancellationToken::new();
    let context = crate::node_system::runtime::RelationalContext {
        run_id: crate::node_system::analysis::RunId::new(1),
        resources: &resources,
        cancellation: &cancellation,
    };
    let plan = crate::node_system::plan::CompiledRelationalPlan {
        fragment_order: Box::new([]),
        operators: Box::new([
            crate::node_system::plan::RelationalOperator::Source {
                resource,
                relation: "main".into(),
            },
            crate::node_system::plan::RelationalOperator::Limit {
                input: crate::node_system::plan::RelationalOperatorIndex::new(0),
                rows: 2,
            },
        ]),
        roots: Box::new([crate::node_system::plan::RelationalOperatorIndex::new(1)]),
        pushdown_hints: Box::new([]),
    };

    let result = ProductionRelationalBackend
        .execute(&context, &plan, &[], &[])
        .unwrap();
    let crate::node_system::runtime::RuntimeValue::Scalar(
        crate::node_system::protocol::Value::Object(columns),
    ) = &result.outputs[0]
    else {
        panic!("expected relational dataframe output")
    };
    assert_eq!(
        columns["value"],
        crate::node_system::protocol::Value::List(vec![
            crate::node_system::protocol::Value::Integer(1),
            crate::node_system::protocol::Value::Integer(2),
        ])
    );
}

#[test]
fn project_execute_graph_lazily_acquires_declared_duckdb_relational_source() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-production-duckdb-run-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(root.join("database")).unwrap();
    crate::project::save_project_to_file(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let duckdb = root.join("database/project.duckdb");
    let mut dataframe = polars::df!("value" => [11_i64, 22, 33]).unwrap();
    crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

    let state = ProjectState::new();
    state.set_path(Some(root.to_string_lossy().into_owned()));
    let event = state
        .create_graph_resource("Production", GraphDocumentKind::Event)
        .unwrap();
    state.load_graph_from_current_project(&event).unwrap();
    state.project_data.write().unwrap().databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: "database/project.duckdb".into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: Some("Main".into()),
        },
    );

    let mut source = node("yssbi.dataframe.source.get");
    source.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );

    state
        .apply_graph_patch(
            &event,
            MutationRequest::new(
                ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                    event.as_str().into(),
                )),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: source }]),
            ),
        )
        .unwrap();

    let result = state.execute_graph(&event, &NOOP_RUN_EVENT_SINK).unwrap();
    assert!(result.run_id.get() > 0);

    let data = state.project_data.read().unwrap();
    let snapshots = crate::project::project_state::snapshot_project_resources(
        &state,
        data.variables.clone(),
        data.databases.clone(),
    )
    .unwrap();
    drop(data);
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(snapshots.runtime);
    use crate::node_system::runtime::ResourceProvider;
    let lease = provider
        .acquire(&crate::node_system::plan::CompiledResourceRequirement {
            resource: crate::node_system::plan::ResourceId::new("databases/main").unwrap(),
            kind: crate::node_system::plan::ResourceKind::DatabaseConnection,
            access: crate::node_system::plan::ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let dataframe = lease
        .as_any()
        .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
        .unwrap()
        .load_dataframe()
        .unwrap()
        .unwrap();
    assert_eq!(
        dataframe
            .column("value")
            .unwrap()
            .i64()
            .unwrap()
            .into_no_null_iter()
            .collect::<Vec<_>>(),
        vec![11, 22, 33]
    );
    assert!(
        !state
            .project_store
            .read()
            .unwrap()
            .databases
            .contains_key("main")
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn production_resource_snapshot_supplies_plot_sink() {
    use crate::node_system::runtime::ResourceProvider;
    let provider = crate::node_system::runtime::ProjectResourceProvider::new(
        crate::node_system::runtime::ProjectResourceSnapshot::new(
            crate::node_system::analysis::ProjectSessionId::new("plot-project"),
            crate::node_system::analysis::ResourceVersionSet::new(),
        )
        .with_plot_sink(std::sync::Arc::new(ProductionPlotSink)),
    );
    let lease = provider
        .acquire(&crate::node_system::plan::CompiledResourceRequirement {
            resource: crate::node_system::plan::ResourceId::new("yssbi.runtime.plot_sink").unwrap(),
            kind: crate::node_system::plan::ResourceKind::ExternalArtifact,
            access: crate::node_system::plan::ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let sink = lease
        .as_any()
        .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
        .unwrap()
        .plot_sink()
        .unwrap();
    assert_eq!(
        sink.publish(crate::node_system::runtime::PlotKind::Line, "payload")
            .unwrap()
            .as_ref(),
        "payload"
    );
}

#[test]
fn project_execution_refuses_blocking_analysis() {
    let state = state_with_empty_graph();
    let invalid = node("yssbi.test.missing");
    let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: invalid }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let error = state
        .execute_graph(&graph_path(), &NOOP_RUN_EVENT_SINK)
        .unwrap_err();
    assert!(error.contains("blocking diagnostics"));
    assert!(error.contains("compiler.node.unknown"));
}

#[test]
fn project_variable_get_executes_against_authoritative_resource() {
    let state = state_with_empty_graph();
    let variable_id = crate::variable::VariableId::new();
    state.project_data.write().unwrap().variables.insert(
        variable_id,
        crate::variable::VariableInstance {
            id: variable_id,
            name: "Rate".into(),
            data_type: crate::graph::value::DataType::Int64,
            data_value: crate::graph::value::DataValue::Int64(9),
            tabular: None,
            description: String::new(),
            scope: crate::variable::VariableScope::Global,
            tags: Vec::new(),
        },
    );
    let mut variable = node("yssbi.project.variable.get");
    variable.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{variable_id}")),
    );
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: variable,
                }]),
            ),
        )
        .unwrap();

    let result = state
        .execute_graph(&graph_path(), &NOOP_RUN_EVENT_SINK)
        .unwrap();
    assert!(result.run_id.get() > 0);
}

#[test]
fn variable_effect_commit_is_revisioned_and_undoable() {
    let state = ProjectState::new();
    let variable = state.add_variable(
        "Rate",
        crate::graph::value::DataType::Int64,
        crate::graph::value::DataValue::Int64(1),
        "",
        crate::variable::VariableScope::Global,
        Vec::new(),
    );
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
                expected_revision: GraphRevision::INITIAL,
                before: variable.clone(),
                after: crate::graph::value::DataValue::Int64(2),
            }],
        )
        .unwrap();
    assert_eq!(committed.variable_ids.as_ref(), &[variable.id]);
    assert_eq!(committed.deltas.len(), 1);
    assert_eq!(committed.deltas[0].from_revision, GraphRevision::INITIAL);
    assert_eq!(committed.deltas[0].to_revision, GraphRevision::new(1));
    assert!(matches!(
        state.get_variable(&variable.id).unwrap().data_value,
        crate::graph::value::DataValue::Int64(2)
    ));

    state
        .undo_last_transaction(MutationRequest::new(
            ResourceKey::Variable(crate::node_system::document::VariableResourceKey(
                format!("variables/{}", variable.id).into(),
            )),
            GraphRevision::new(1),
            OperationId::new(),
            HistoryMutation {},
        ))
        .unwrap();
    assert!(matches!(
        state.get_variable(&variable.id).unwrap().data_value,
        crate::graph::value::DataValue::Int64(1)
    ));
}

#[test]
fn concurrent_variable_effect_commit_returns_structured_revision_conflict() {
    let state = ProjectState::new();
    let variable = state.add_variable(
        "Rate",
        crate::graph::value::DataType::Int64,
        crate::graph::value::DataValue::Int64(1),
        "",
        crate::variable::VariableScope::Global,
        Vec::new(),
    );
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
        expected_revision: GraphRevision::INITIAL,
        before: variable.clone(),
        after: crate::graph::value::DataValue::Int64(2),
    };
    let winning_effect = crate::node_system::runtime::VariableWriteEffect {
        after: crate::graph::value::DataValue::Int64(3),
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
        state.get_variable(&variable.id).unwrap().data_value,
        crate::graph::value::DataValue::Int64(3)
    ));
}

#[test]
fn project_execution_runs_valid_plan_through_run_executor() {
    let state = state_with_empty_graph();
    let mut constant = node("yssbi.constant.int64");
    constant.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(7),
    );
    let patch =
        GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: constant }]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let result = state
        .execute_graph(&graph_path(), &NOOP_RUN_EVENT_SINK)
        .unwrap();
    assert!(result.run_id.get() > 0);
}
