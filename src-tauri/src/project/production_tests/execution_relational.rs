use super::*;

struct SourceRenameLimitFixture {
    state: ProjectState,
    root: std::path::PathBuf,
    path: GraphResourcePath,
    nodes: [DocumentNode; 3],
    connections: [crate::graph_document::DocumentConnection; 2],
    rename_result_name: String,
    limit_result_name: String,
}

impl SourceRenameLimitFixture {
    fn new(label: &str) -> Self {
        use crate::graph_document::{ConnectionId, DocumentConnection, PortAddress};
        use crate::node_system::protocol::{ParameterKey, PortKey};

        let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("database")).unwrap();
        let duckdb = root.join("database/project.duckdb");
        let mut dataframe = polars::df!(
            "old_name" => [11_i64, 22, 33, 44],
            "untouched" => [101_i64, 202, 303, 404],
        )
        .unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

        let mut project_data = ProjectData::new();
        project_data.databases.insert(
            "main".into(),
            crate::database::DatabaseDecl {
                id: "main".into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: "Main".into(),
            },
        );
        crate::project::fixtures::write_project(&project_data, root.to_string_lossy().as_ref())
            .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

        let mut source = node("yssbi.dataframe.source.get");
        source.parameters.insert(
            ParameterKey::new("dataframe").unwrap(),
            serde_json::json!("databases/main"),
        );
        let mut rename = node("yssbi.dataframe.rename");
        rename.parameters.insert(
            ParameterKey::new("from").unwrap(),
            serde_json::json!("old_name"),
        );
        rename.parameters.insert(
            ParameterKey::new("to").unwrap(),
            serde_json::json!("new_name"),
        );
        let mut limit = node("yssbi.dataframe.limit");
        limit
            .parameters
            .insert(ParameterKey::new("rows").unwrap(), serde_json::json!(2));
        let rename_result_name = format!("node.{}.result", rename.id);
        let limit_result_name = format!("node.{}.result", limit.id);

        let source_to_rename = DocumentConnection {
            id: ConnectionId::new(),
            output: PortAddress::declared(source.id, PortKey::new("dataframe").unwrap()),
            input: PortAddress::declared(rename.id, PortKey::new("source").unwrap()),
            order: None,
        };
        let rename_to_limit = DocumentConnection {
            id: ConnectionId::new(),
            output: PortAddress::declared(rename.id, PortKey::new("result").unwrap()),
            input: PortAddress::declared(limit.id, PortKey::new("source").unwrap()),
            order: None,
        };

        Self {
            state,
            root,
            path: GraphResourcePath::new("events/SourceRenameLimit.yssbi-event").unwrap(),
            nodes: [source, rename, limit],
            connections: [source_to_rename, rename_to_limit],
            rename_result_name,
            limit_result_name,
        }
    }

    fn document(&self, reversed: bool) -> GraphResourceDocument {
        let mut graph = GraphResourceDocument::new("Source Rename Limit", GraphDocumentKind::Event);
        if reversed {
            for node in self.nodes.iter().rev() {
                graph.document.nodes.insert(node.id, node.clone());
            }
            for connection in self.connections.iter().rev() {
                graph
                    .document
                    .connections
                    .insert(connection.id, connection.clone());
            }
        } else {
            for node in &self.nodes {
                graph.document.nodes.insert(node.id, node.clone());
            }
            for connection in &self.connections {
                graph
                    .document
                    .connections
                    .insert(connection.id, connection.clone());
            }
        }
        graph
    }
}

#[test]
fn project_execute_graph_runs_builtin_dataframe_source_rename_limit() {
    use crate::node_system::plan::{
        RelationalOperator, RelationalOperatorIndex, RelationalRename, ResourceId,
    };
    use crate::node_system::protocol::Value;
    use crate::node_system::runtime::{ProductionRelationalObserver, RuntimeValue};

    let fixture = SourceRenameLimitFixture::new("project-source-rename-limit");
    fixture
        .state
        .insert_graph(fixture.path.clone(), fixture.document(false))
        .unwrap();
    let observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    fixture
        .state
        .set_production_relational_observer(std::sync::Arc::clone(&observer));

    let result = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("authoritative Source -> Rename -> Limit graph executes");

    let observation = observer.snapshot();
    assert_eq!(observation.relational_islands, Some(1));
    assert_eq!(observation.backend_invocations, 1);
    assert_eq!(observation.relational_subplans.len(), 1);
    let plan = &observation.relational_subplans[0].compiled_plan;
    assert_eq!(
        plan.operators.as_ref(),
        &[
            RelationalOperator::Source {
                resource: ResourceId::new("databases/main").unwrap(),
                relation: "databases/main".into(),
            },
            RelationalOperator::Rename {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalRename {
                    from: "old_name".into(),
                    to: "new_name".into(),
                }]),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(1),
                rows: 2,
            },
        ]
    );
    assert_eq!(
        plan.roots.as_ref(),
        &[
            RelationalOperatorIndex::new(1),
            RelationalOperatorIndex::new(2),
        ]
    );
    assert_eq!(
        observation.relational_result_bindings,
        vec![
            (
                fixture.rename_result_name.as_str().into(),
                RelationalOperatorIndex::new(1),
            ),
            (
                fixture.limit_result_name.as_str().into(),
                RelationalOperatorIndex::new(2),
            ),
        ]
    );

    let RuntimeValue::Scalar(Value::Object(rename_columns)) = result
        .value_for_test(fixture.rename_result_name.as_str())
        .expect("Rename result must be exposed")
    else {
        panic!("expected Rename dataframe output")
    };
    assert!(!rename_columns.contains_key("old_name"));
    assert_eq!(
        rename_columns,
        [
            (
                "new_name".into(),
                Value::List(vec![
                    Value::Integer(11),
                    Value::Integer(22),
                    Value::Integer(33),
                    Value::Integer(44),
                ]),
            ),
            (
                "untouched".into(),
                Value::List(vec![
                    Value::Integer(101),
                    Value::Integer(202),
                    Value::Integer(303),
                    Value::Integer(404),
                ]),
            ),
        ]
        .into_iter()
        .collect()
    );

    let RuntimeValue::Scalar(Value::Object(limit_columns)) = result
        .value_for_test(fixture.limit_result_name.as_str())
        .expect("Limit result must be exposed")
    else {
        panic!("expected Limit dataframe output")
    };
    assert!(!limit_columns.contains_key("old_name"));
    assert_eq!(
        limit_columns,
        [
            (
                "new_name".into(),
                Value::List(vec![Value::Integer(11), Value::Integer(22)]),
            ),
            (
                "untouched".into(),
                Value::List(vec![Value::Integer(101), Value::Integer(202)]),
            ),
        ]
        .into_iter()
        .collect()
    );
    assert!(plan.pushdown_hints.is_empty());
    assert_eq!(observation.scan_limits, vec![None]);

    drop(fixture.state);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn project_execute_graph_source_rename_limit_is_insertion_order_independent() {
    use crate::node_system::runtime::ProductionRelationalObserver;

    let fixture = SourceRenameLimitFixture::new("project-source-rename-limit-order");
    let forward_document = fixture.document(false);
    let reversed_document = fixture.document(true);
    assert_eq!(forward_document.document, reversed_document.document);

    fixture
        .state
        .insert_graph(fixture.path.clone(), forward_document)
        .unwrap();
    let forward_observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    fixture
        .state
        .set_production_relational_observer(std::sync::Arc::clone(&forward_observer));
    let forward = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("forward insertion graph executes");

    fixture
        .state
        .insert_graph(fixture.path.clone(), reversed_document)
        .unwrap();
    let reversed_observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    fixture
        .state
        .set_production_relational_observer(std::sync::Arc::clone(&reversed_observer));
    let mut reversed = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("reversed insertion graph executes");

    assert_eq!(forward_observer.snapshot(), reversed_observer.snapshot());
    assert_ne!(forward.run_id, reversed.run_id);
    assert_ne!(
        forward.provenance.compile_id,
        reversed.provenance.compile_id
    );
    reversed.run_id = forward.run_id;
    reversed.provenance.compile_id = forward.provenance.compile_id;
    assert_eq!(
        forward.result_ids.keys().collect::<Vec<_>>(),
        reversed.result_ids.keys().collect::<Vec<_>>()
    );
    assert!(
        forward
            .result_ids
            .values()
            .all(|id| !reversed.result_ids.values().any(|other| other == id))
    );
    reversed.result_ids = forward.result_ids.clone();
    assert_eq!(forward, reversed);

    drop(fixture.state);
    std::fs::remove_dir_all(fixture.root).unwrap();
}

#[test]
fn project_execution_preserves_relational_codes_in_errors_and_terminal_events() {
    struct FailingBackend(crate::node_system::runtime::RelationalErrorCode);

    impl crate::node_system::runtime::RelationalBackend for FailingBackend {
        fn execute(
            &self,
            _: &crate::node_system::runtime::RelationalContext<'_>,
            _: &crate::node_system::plan::CompiledRelationalPlan,
            _: &[crate::node_system::runtime::RuntimeValue],
        ) -> Result<
            crate::node_system::runtime::RelationalExecution,
            crate::node_system::runtime::RelationalError,
        > {
            Err(crate::node_system::runtime::RelationalError::new(
                self.0,
                "sensitive backend detail",
            ))
        }
    }

    for (relational_code, run_code, public_message) in [
        (
            crate::node_system::runtime::RelationalErrorCode::HintInvalid,
            crate::node_system::runtime::RunErrorCode::RelationalHintInvalid,
            "relational pushdown metadata is invalid",
        ),
        (
            crate::node_system::runtime::RelationalErrorCode::TypeMismatch,
            crate::node_system::runtime::RunErrorCode::RelationalTypeMismatch,
            "relational types do not match",
        ),
    ] {
        let fixture = SourceRenameLimitFixture::new("project-relational-error-code");
        fixture
            .state
            .insert_graph(fixture.path.clone(), fixture.document(false))
            .unwrap();
        fixture
            .state
            .set_production_relational_backend_factory(std::sync::Arc::new(move || {
                std::sync::Arc::new(FailingBackend(relational_code))
                    as std::sync::Arc<dyn crate::node_system::runtime::RelationalBackend>
            }));
        let events = DemandRunEvents::default();

        let error = fixture
            .state
            .execute_graph_for_current_project_for_test(
                &fixture.path,
                &crate::node_system::plan::ExecutionDemand::Default,
                &events,
            )
            .unwrap_err();

        assert_eq!(error.to_string(), public_message);
        assert!(!error.to_string().contains("sensitive backend detail"));
        assert!(matches!(
            error.run_error(),
            Some(crate::node_system::runtime::RunError::RelationalFailed {
                code,
                ..
            }) if *code == relational_code
        ));
        let events = events.0.lock().unwrap();
        assert!(events.iter().any(|event| matches!(
            event.kind,
            crate::node_system::runtime::RunEventKind::RunErrored { outcome }
                if outcome.code() == run_code
        )));
        assert!(events.iter().all(|event| !matches!(
            event.kind,
            crate::node_system::runtime::RunEventKind::RunCompleted
        )));

        drop(events);
        drop(fixture.state);
        std::fs::remove_dir_all(fixture.root).unwrap();
    }
}

#[test]
fn project_execute_graph_runs_builtin_dataframe_source_limit() {
    use crate::graph_document::{ConnectionId, DocumentConnection, PortAddress};
    use crate::node_system::protocol::{ParameterKey, PortKey, Value};
    use crate::node_system::runtime::{ProductionRelationalObserver, RuntimeValue};

    let root = std::env::temp_dir().join(format!(
        "yssbi-project-relational-e2e-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(root.join("database")).unwrap();
    let duckdb = root.join("database/project.duckdb");
    let mut dataframe = polars::df!("value" => [11_i64, 22, 33, 44]).unwrap();
    crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

    let mut project_data = ProjectData::new();
    project_data.databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::DuckDb {
                path: "database/project.duckdb".into(),
                table: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: "Main".into(),
        },
    );
    crate::project::fixtures::write_project(&project_data, root.to_string_lossy().as_ref())
        .unwrap();
    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

    let mut source = node("yssbi.dataframe.source.get");
    source.parameters.insert(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );
    let mut limit = node("yssbi.dataframe.limit");
    let result_name = format!("node.{}.result", limit.id);
    limit
        .parameters
        .insert(ParameterKey::new("rows").unwrap(), serde_json::json!(2));
    let connection_id = ConnectionId::new();
    let connection = DocumentConnection {
        id: connection_id,
        output: PortAddress::declared(source.id, PortKey::new("dataframe").unwrap()),
        input: PortAddress::declared(limit.id, PortKey::new("source").unwrap()),
        order: None,
    };
    let mut graph = GraphResourceDocument::new("Relational", GraphDocumentKind::Event);
    graph.document.nodes.insert(source.id, source);
    graph.document.nodes.insert(limit.id, limit);
    graph.document.connections.insert(connection_id, connection);
    let path = GraphResourcePath::new("events/Relational.yssbi-event").unwrap();
    state.insert_graph(path.clone(), graph).unwrap();

    let observer = std::sync::Arc::new(ProductionRelationalObserver::default());
    state.set_production_relational_observer(std::sync::Arc::clone(&observer));

    let result = state
        .execute_graph_for_current_project_for_test(
            &path,
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .expect("authoritative relational graph executes");

    let observation = observer.snapshot();
    assert_eq!(observation.relational_islands, Some(1));
    assert_eq!(observation.backend_invocations, 1);
    assert_eq!(observation.scan_limits, vec![Some(2)]);
    assert_eq!(
        result.value_for_test(result_name.as_str()),
        Some(RuntimeValue::Scalar(Value::Object(
            [(
                "value".into(),
                Value::List(vec![Value::Integer(11), Value::Integer(22)]),
            )]
            .into_iter()
            .collect(),
        )))
    );

    drop(state);
    std::fs::remove_dir_all(root).unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProductionChainOutput {
    Filter,
    Limit,
}

struct ProductionRelationalChainFixture {
    state: ProjectState,
    root: std::path::PathBuf,
    path: GraphResourcePath,
    nodes: [DocumentNode; 5],
}

impl ProductionRelationalChainFixture {
    fn new(label: &str, reverse_uuid_order: bool) -> Self {
        use crate::graph_document::{ConnectionId, DocumentConnection, NodeId, PortAddress};
        use crate::node_system::protocol::{ParameterKey, PortKey};

        let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("database")).unwrap();
        let duckdb = root.join("database/project.duckdb");
        let mut dataframe = polars::df!(
            "amount" => [10_i64, 20, 30, 40, 50],
            "region" => [Some("east"), None, Some("west"), Some("north"), Some("south")],
            "active" => [true, false, true, false, true],
        )
        .unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &duckdb, "main").unwrap();

        let ids = if reverse_uuid_order {
            [5_u128, 4, 3, 2, 1]
        } else {
            [1_u128, 2, 3, 4, 5]
        };
        let mut source = node("yssbi.dataframe.source.get");
        source.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[0]));
        source.parameters.insert(
            ParameterKey::new("dataframe").unwrap(),
            serde_json::json!("databases/main"),
        );
        let mut filter = node("yssbi.dataframe.filter.rows");
        filter.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[1]));
        filter.parameters.insert(
            ParameterKey::new("predicate").unwrap(),
            serde_json::json!({
                "column": "amount",
                "operator": "greaterThan",
                "value": { "type": "integer", "value": "10" }
            }),
        );
        let mut project = node("yssbi.dataframe.project");
        project.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[2]));
        project.parameters.insert(
            ParameterKey::new("columns").unwrap(),
            serde_json::json!(["amount", "region"]),
        );
        let mut rename = node("yssbi.dataframe.rename");
        rename.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[3]));
        rename.parameters.insert(
            ParameterKey::new("from").unwrap(),
            serde_json::json!("amount"),
        );
        rename.parameters.insert(
            ParameterKey::new("to").unwrap(),
            serde_json::json!("selected_amount"),
        );
        let mut limit = node("yssbi.dataframe.limit");
        limit.id = NodeId::from_uuid(uuid::Uuid::from_u128(ids[4]));
        limit
            .parameters
            .insert(ParameterKey::new("rows").unwrap(), serde_json::json!(2));
        let nodes = [source, filter, project, rename, limit];

        let mut graph = GraphResourceDocument::new("Production chain", GraphDocumentKind::Event);
        for node in nodes.iter().rev() {
            graph.document.nodes.insert(node.id, node.clone());
        }
        let links = [
            (0, "dataframe", 1, "source"),
            (1, "result", 2, "source"),
            (2, "result", 3, "source"),
            (3, "result", 4, "source"),
        ];
        for (offset, (output_node, output, input_node, input)) in links.into_iter().enumerate() {
            let id = ConnectionId::from_uuid(uuid::Uuid::from_u128(100 + offset as u128));
            graph.document.connections.insert(
                id,
                DocumentConnection {
                    id,
                    output: PortAddress::declared(
                        nodes[output_node].id,
                        PortKey::new(output).unwrap(),
                    ),
                    input: PortAddress::declared(
                        nodes[input_node].id,
                        PortKey::new(input).unwrap(),
                    ),
                    order: None,
                },
            );
        }

        let path = GraphResourcePath::new("events/ProductionChain.yssbi-event").unwrap();
        let mut project_data = ProjectData::new();
        project_data.databases.insert(
            "main".into(),
            crate::database::DatabaseDecl {
                id: "main".into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/project.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: "Main".into(),
            },
        );
        project_data.graphs.insert(path.clone(), graph);
        crate::project::fixtures::write_project(&project_data, root.to_string_lossy().as_ref())
            .unwrap();
        crate::project::fixtures::write_graph(
            &project_data,
            root.to_string_lossy().as_ref(),
            &path,
        )
        .unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project_data);

        let fixture = Self {
            state,
            root,
            path,
            nodes,
        };
        fixture.assert_persisted_parameters();
        fixture
    }

    fn node_id(&self, output: ProductionChainOutput) -> crate::graph_document::NodeId {
        self.nodes[match output {
            ProductionChainOutput::Filter => 1,
            ProductionChainOutput::Limit => 4,
        }]
        .id
    }

    fn output_ref(
        &self,
        output: ProductionChainOutput,
    ) -> crate::node_system::plan::GraphOutputRef {
        crate::node_system::plan::GraphOutputRef {
            graph_path: self.path.clone(),
            port: crate::graph_document::PortAddress::declared(
                self.node_id(output),
                crate::node_system::protocol::PortKey::new("result").unwrap(),
            ),
        }
    }

    fn demand(&self, output: ProductionChainOutput) -> crate::node_system::plan::ExecutionDemand {
        crate::node_system::plan::ExecutionDemand::Outputs {
            outputs: Box::new([self.output_ref(output)]),
            include_default_results: false,
        }
    }

    fn install(
        &self,
        observer: &std::sync::Arc<crate::node_system::runtime::ProductionRelationalObserver>,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        self.state
            .set_production_relational_observer(std::sync::Arc::clone(observer));
        self.state
            .set_project_resource_lease_observer(leases.clone());
    }

    fn assert_persisted_parameters(&self) {
        use crate::node_system::protocol::ParameterKey;

        let reloaded = ProjectState::new();
        reloaded.activate_project_from_path(&self.root).unwrap();
        let loaded = load_graph(&reloaded, &self.path).unwrap();
        let graph = &loaded.document;
        assert_eq!(
            graph.nodes[&self.nodes[0].id].parameters,
            [(
                ParameterKey::new("dataframe").unwrap(),
                serde_json::json!("databases/main")
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[1].id].parameters,
            [(
                ParameterKey::new("predicate").unwrap(),
                serde_json::json!({
                    "column": "amount",
                    "operator": "greaterThan",
                    "value": { "type": "integer", "value": "10" }
                })
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[2].id].parameters,
            [(
                ParameterKey::new("columns").unwrap(),
                serde_json::json!(["amount", "region"])
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[3].id].parameters,
            [
                (
                    ParameterKey::new("from").unwrap(),
                    serde_json::json!("amount")
                ),
                (
                    ParameterKey::new("to").unwrap(),
                    serde_json::json!("selected_amount"),
                ),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            graph.nodes[&self.nodes[4].id].parameters,
            [(ParameterKey::new("rows").unwrap(), serde_json::json!(2))]
                .into_iter()
                .collect()
        );
    }

    fn expected_value(output: ProductionChainOutput) -> crate::node_system::runtime::RuntimeValue {
        use crate::node_system::protocol::Value;
        use crate::node_system::runtime::RuntimeValue;

        let amount_name = if output == ProductionChainOutput::Limit {
            "selected_amount"
        } else {
            "amount"
        };
        let amount_values = if output == ProductionChainOutput::Limit {
            vec![Value::Integer(20), Value::Integer(30)]
        } else {
            vec![
                Value::Integer(20),
                Value::Integer(30),
                Value::Integer(40),
                Value::Integer(50),
            ]
        };
        let region_values = if output == ProductionChainOutput::Limit {
            vec![Value::Null, Value::String("west".into())]
        } else {
            vec![
                Value::Null,
                Value::String("west".into()),
                Value::String("north".into()),
                Value::String("south".into()),
            ]
        };
        let mut columns = std::collections::BTreeMap::from([
            (amount_name.into(), Value::List(amount_values)),
            ("region".into(), Value::List(region_values)),
        ]);
        if output == ProductionChainOutput::Filter {
            columns.insert(
                "active".into(),
                Value::List(vec![
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Bool(false),
                    Value::Bool(true),
                ]),
            );
        }
        RuntimeValue::Scalar(Value::Object(columns))
    }

    fn assert_common_success(
        &self,
        output: ProductionChainOutput,
        result: &crate::node_system::runtime::RunResult,
        events: &DemandRunEvents,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        assert_eq!(result.result_ids.len(), 1);
        let result_name = result.result_ids.keys().next().expect("one result name");
        assert_eq!(
            result.value_for_test(result_name),
            Some(Self::expected_value(output))
        );
        let _expected_name = format!("node.{}.result", self.node_id(output));
        let events = events.0.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    crate::node_system::runtime::RunEventKind::RunCompleted
                ))
                .count(),
            1
        );

        drop(events);
        let store = self.state.project_store.read().unwrap();
        assert_eq!(store.runs.active_run_count(), 0);

        drop(store);
        assert_eq!(leases.acquired(), 1);
        assert_eq!(leases.dropped(), 1);
        assert_eq!(leases.active(), 0);
    }

    fn assert_final_only_acceptance(
        &self,
        result: &crate::node_system::runtime::RunResult,
        events: &DemandRunEvents,
        observer: &crate::node_system::runtime::ProductionRelationalObserver,
        leases: &crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        use crate::node_system::plan::{RelationalOperator, RelationalOperatorIndex};

        self.assert_common_success(ProductionChainOutput::Limit, result, events, leases);
        let observation = observer.snapshot();
        assert_eq!(observation.relational_islands, Some(1));
        assert_eq!(observation.backend_invocations, 1);
        assert_eq!(observation.relational_subplans.len(), 1);
        let plan = &observation.relational_subplans[0].compiled_plan;
        assert_eq!(plan.operators.len(), 5);
        assert_eq!(plan.roots.as_ref(), &[RelationalOperatorIndex::new(4)]);
        assert!(matches!(
            plan.operators[0],
            RelationalOperator::Source { .. }
        ));
        assert!(
            matches!(plan.operators[1], RelationalOperator::Filter { input, .. } if input == RelationalOperatorIndex::new(0))
        );
        assert!(
            matches!(plan.operators[2], RelationalOperator::Project { input, .. } if input == RelationalOperatorIndex::new(1))
        );
        assert!(
            matches!(plan.operators[3], RelationalOperator::Rename { input, .. } if input == RelationalOperatorIndex::new(2))
        );
        assert!(
            matches!(plan.operators[4], RelationalOperator::Limit { input, rows: 2 } if input == RelationalOperatorIndex::new(3))
        );

        let dataframes = observer.materialized_dataframes();
        assert_eq!(dataframes.len(), 1);
        let dataframe = &dataframes[0];
        assert_eq!(
            dataframe
                .get_column_names()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["selected_amount", "region"]
        );
        assert_eq!(
            dataframe.column("selected_amount").unwrap().dtype(),
            &polars::prelude::DataType::Int64
        );
        assert_eq!(
            dataframe.column("region").unwrap().dtype(),
            &polars::prelude::DataType::String
        );
        assert_eq!(dataframe.column("selected_amount").unwrap().null_count(), 0);
        assert_eq!(dataframe.column("region").unwrap().null_count(), 1);
        assert_eq!(
            dataframe
                .column("selected_amount")
                .unwrap()
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            vec![20, 30]
        );
        assert_eq!(
            dataframe
                .column("region")
                .unwrap()
                .str()
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec![None, Some("west")]
        );
    }

    fn cleanup(self) {
        drop(self.state);
        std::fs::remove_dir_all(self.root).unwrap();
    }
}

#[test]
fn production_relational_chain_persisted_parameters_reload_from_disk_authority() {
    use crate::node_system::protocol::ParameterKey;

    let fixture = ProductionRelationalChainFixture::new("persisted-reload", false);
    {
        let mut data = fixture.state.project_data.write().unwrap();
        let graph = &mut data.graphs.get_mut(&fixture.path).unwrap().document;
        graph
            .nodes
            .get_mut(&fixture.nodes[1].id)
            .unwrap()
            .parameters
            .insert(
                ParameterKey::new("predicate").unwrap(),
                serde_json::json!({ "column": "corrupted" }),
            );
        graph
            .nodes
            .get_mut(&fixture.nodes[2].id)
            .unwrap()
            .parameters
            .insert(
                ParameterKey::new("columns").unwrap(),
                serde_json::json!(["corrupted"]),
            );
    }

    fixture.assert_persisted_parameters();
    fixture.cleanup();
}

#[test]
fn production_relational_chain_final_only_demand_publishes_only_exact_final_value() {
    let fixture = ProductionRelationalChainFixture::new("final-only", false);
    let events = DemandRunEvents::default();
    let observer =
        std::sync::Arc::new(crate::node_system::runtime::ProductionRelationalObserver::default());
    let leases = crate::node_system::runtime::ProjectResourceLeaseObserver::default();
    fixture.install(&observer, &leases);

    let result = fixture
        .state
        .execute_graph_for_current_project_for_test(
            &fixture.path,
            &fixture.demand(ProductionChainOutput::Limit),
            &events,
        )
        .expect("final-only production chain executes");

    fixture.assert_final_only_acceptance(&result, &events, &observer, &leases);
    fixture.cleanup();
}
