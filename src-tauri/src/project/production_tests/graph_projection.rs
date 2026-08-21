use super::*;

fn dataframe_decompose_production_fixture(
    include_database: bool,
) -> (ProjectState, std::path::PathBuf, NodeId, NodeId) {
    let root = std::env::temp_dir().join(format!(
        "yssbi-dataframe-decompose-production-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();

    let source_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x400));
    let decompose_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x401));
    let connection_id = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x402));
    let consumer_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x403));
    let mut source = node("yssbi.dataframe.source.get");
    source.id = source_id;
    source.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );
    let mut decompose = node("yssbi.dataframe.decompose");
    decompose.id = decompose_id;
    let mut consumer = node("yssbi.debug.view");
    consumer.id = consumer_id;
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    graph.document.nodes.insert(source_id, source);
    graph.document.nodes.insert(decompose_id, decompose);
    graph.document.nodes.insert(consumer_id, consumer);
    graph.document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(source_id, PortKey::new("dataframe").unwrap()),
            input: PortAddress::declared(decompose_id, PortKey::new("dataframe").unwrap()),
            order: None,
        },
    );

    let mut data = ProjectData::new();
    data.graphs.insert(graph_path(), graph);
    if include_database {
        std::fs::create_dir_all(root.join("database")).unwrap();
        let database_path = root.join("database/main.duckdb");
        let mut dataframe = polars::df!("customer_id" => [1_i64], "amount" => [2.5_f64]).unwrap();
        crate::database::ingest_dataframe_to_duckdb(&mut dataframe, &database_path, "main")
            .unwrap();
        data.databases.insert(
            "main".into(),
            crate::database::DatabaseDecl {
                id: "main".into(),
                engine: crate::database::DatabaseEngine::DuckDb {
                    path: "database/main.duckdb".into(),
                    table: "main".into(),
                },
                schema_version: 1,
                required: true,
                name: "Main".into(),
            },
        );
    }

    let state = ProjectState::new();
    state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
    (state, root, decompose_id, consumer_id)
}

#[test]
fn production_decompose_projects_database_column_metadata() {
    let mut data = ProjectData::new();
    data.databases.insert(
        "main".into(),
        crate::database::DatabaseDecl {
            id: "main".into(),
            engine: crate::database::DatabaseEngine::InMemory {
                name: "main".into(),
            },
            schema_version: 1,
            required: true,
            name: "Main".into(),
        },
    );
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let resources = compile_resources_from_data(
        &data,
        std::collections::BTreeMap::from([(
            resource,
            vec![
                crate::schema::ColumnInfoDTO {
                    name: "customer_id".into(),
                    dtype: "Int64".into(),
                },
                crate::schema::ColumnInfoDTO {
                    name: "amount".into(),
                    dtype: "Float64".into(),
                },
                crate::schema::ColumnInfoDTO {
                    name: "opaque".into(),
                    dtype: "Binary".into(),
                },
            ],
        )]),
    )
    .unwrap();
    let builtin = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let source_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x410));
    let decompose_id = NodeId::from_uuid(uuid::Uuid::from_u128(0x411));
    let mut source = node("yssbi.dataframe.source.get");
    source.id = source_id;
    source.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );
    let mut decompose = node("yssbi.dataframe.decompose");
    decompose.id = decompose_id;
    let connection_id = ConnectionId::from_uuid(uuid::Uuid::from_u128(0x412));
    let mut document = crate::node_system::document::GraphDocument::default();
    document.nodes.insert(source_id, source);
    document.nodes.insert(decompose_id, decompose);
    document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(source_id, PortKey::new("dataframe").unwrap()),
            input: PortAddress::declared(decompose_id, PortKey::new("dataframe").unwrap()),
            order: None,
        },
    );
    let compiler = crate::node_system::compiler::GraphCompiler::with_resolvers(
        builtin.registry.as_ref(),
        &resources,
        resources.schema_resolvers(),
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    );
    let result = compiler.compile(&document);
    let projection = crate::node_system::analysis::build_editor_graph_projection(
        "events/production-dataframe-metadata.yssbi-event",
        &document,
        &result.analysis,
        &result.outcome,
        builtin.registry.as_ref(),
        &builtin.catalog.localization("en-US"),
    )
    .unwrap();
    let node = projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == decompose_id.to_string())
        .unwrap();
    let ports = node
        .ports
        .iter()
        .filter(|port| port.template_key.as_ref() == "columns")
        .collect::<Vec<_>>();

    assert_eq!(
        ports
            .iter()
            .map(|port| port.display.instance_label.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["customer_id", "amount", "opaque"],
    );
    assert_eq!(
        ports[..2]
            .iter()
            .map(|port| {
                let summary = port.resolved_type.as_ref().unwrap();
                assert!(summary.resolved);
                summary.data_type.clone().unwrap()
            })
            .collect::<Vec<_>>(),
        vec![
            crate::graph::DataType::DataSeries(Box::new(crate::graph::DataType::Int64)),
            crate::graph::DataType::DataSeries(Box::new(crate::graph::DataType::Float64)),
        ],
    );
    let opaque = ports[2];
    let opaque_type = opaque.resolved_type.as_ref().unwrap();
    assert!(!opaque_type.resolved);
    assert_eq!(opaque_type.data_type, None);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.dataframe.field_type_unsupported"
            && diagnostic.arguments.get("column").map(Box::as_ref) == Some("opaque")
            && matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Port(address)
                    if crate::node_system::analysis::PortAddressDto::from(address) == opaque.address
            )
    }));
}

#[test]
fn project_compile_resolves_dataframe_decompose_columns() {
    let (state, root, decompose_id, _) = dataframe_decompose_production_fixture(true);
    let source = state.capture_projection_source(&graph_path()).unwrap();
    let (analysis, _) = state
        .get_or_compile_current_from_source(&graph_path(), &source)
        .unwrap();
    let diagnostic_codes = analysis
        .payload
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    let decompose_candidate_count = analysis
        .payload
        .analysis
        .resolved_interfaces
        .iter()
        .find(|interface| interface.node_id == decompose_id)
        .unwrap()
        .ports
        .iter()
        .filter(|port| port.template.as_str() == "columns" && port.address.is_instance())
        .count();
    let decompose_input_schema_labels = analysis
        .payload
        .analysis
        .resolved_schemas
        .get(&PortAddress::declared(
            decompose_id,
            PortKey::new("dataframe").unwrap(),
        ))
        .unwrap()
        .fields
        .iter()
        .map(|field| field.name.0.as_ref())
        .collect::<Vec<_>>();

    assert!(!diagnostic_codes.contains(&"compiler.interface.resolver_missing"));
    assert_eq!(
        decompose_candidate_count,
        decompose_input_schema_labels.len()
    );
    assert_eq!(decompose_input_schema_labels, vec!["customer_id", "amount"]);
    assert!(
        analysis
            .payload
            .analysis
            .basis
            .resource_versions
            .contains_key(&crate::node_system::analysis::ResourceKey::new(
                "databases/main"
            ))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn editor_connect_materializes_current_decompose_projection_and_preserves_orphan() {
    let (state, root, decompose_id, consumer_id) = dataframe_decompose_production_fixture(true);
    let project_instance_id = current_project_instance_id(&state);
    let source = state.capture_projection_source(&graph_path()).unwrap();
    let (analysis, _) = state
        .get_or_compile_current_from_source(&graph_path(), &source)
        .unwrap();
    let members = &analysis
        .payload
        .interface_projection
        .nodes
        .get(&decompose_id)
        .unwrap()
        .available_members;
    let projected = members
        .iter()
        .find(|candidate| candidate.member().label == "customer_id")
        .unwrap()
        .projection_address()
        .clone();
    let other_projected = members
        .iter()
        .find(|candidate| candidate.member().label == "amount")
        .unwrap()
        .projection_address()
        .clone();
    let two_projected = state
        .apply_editor_graph_mutation(
            &project_instance_id,
            &graph_path(),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::INITIAL,
                OperationId::from_uuid(uuid::Uuid::from_u128(0x406)),
                EditorGraphMutationDto::Connect {
                    output: crate::node_system::document::PortAddressDto::from(projected.clone()),
                    input: crate::node_system::document::PortAddressDto::from(other_projected),
                    order: None,
                },
            ),
        )
        .unwrap_err();
    assert!(
        two_projected
            .to_string()
            .contains("two projected members are not supported")
    );
    assert_eq!(
        state.get_data().unwrap().graphs[&graph_path()]
            .document
            .revision,
        GraphRevision::INITIAL,
    );
    let mut observed = Vec::new();

    let request = MutationRequest::new(
        ResourceKey::Graph(document_path()),
        GraphRevision::INITIAL,
        OperationId::from_uuid(uuid::Uuid::from_u128(0x404)),
        EditorGraphMutationDto::Connect {
            output: crate::node_system::document::PortAddressDto::from(projected),
            input: crate::node_system::document::PortAddressDto::from(PortAddress::declared(
                consumer_id,
                PortKey::new("data").unwrap(),
            )),
            order: None,
        },
    );
    let result = crate::commands::command_node_system::mutate_graph_document_with_emitter(
        &state,
        project_instance_id.clone(),
        graph_path().as_str().to_string(),
        "en-US",
        serde_json::to_value(request).unwrap(),
        |event| observed.push(event),
    )
    .expect("a real command EditorGraphMutationDto Connect materializes its current endpoint");

    assert!(matches!(
        observed.as_slice(),
        [crate::event::Event::Project(crate::event::EventProject::GraphDelta { delta, .. })]
            if delta == &result.delta
    ));
    let document = &state.get_data().unwrap().graphs[&graph_path()].document;
    let (materialized, binding) = document
        .port_bindings
        .iter()
        .find(|(_, binding)| matches!(
            binding,
            crate::node_system::document::DynamicPortBinding::Resolved { origin, .. }
                if matches!(origin, crate::node_system::document::DynamicMemberLocator::SchemaField { field, .. } if field.0.as_ref() == "customer_id")
        ))
        .unwrap();
    let expected_metadata = crate::node_system::document::LastKnownPortMetadata {
        label: "customer_id".into(),
        value_type: Some(crate::node_system::protocol::data_series_type(
            crate::node_system::protocol::TypeExpr::Concrete(
                crate::node_system::protocol::TypeId::new("core.int64").unwrap(),
            ),
        )),
    };
    assert!(matches!(
        binding,
        crate::node_system::document::DynamicPortBinding::Resolved { last_known, .. }
            if last_known == &expected_metadata
    ));
    assert!(document.connections.values().any(|connection| {
        connection.output == *materialized
            && connection.input == PortAddress::declared(consumer_id, PortKey::new("data").unwrap())
    }));
    let materialized = materialized.clone();
    let stale = state
        .apply_editor_graph_mutation(
            &project_instance_id,
            &graph_path(),
            "en-US",
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                GraphRevision::new(1),
                OperationId::from_uuid(uuid::Uuid::from_u128(0x407)),
                EditorGraphMutationDto::Connect {
                    output: crate::node_system::document::PortAddressDto::from(
                        members
                            .iter()
                            .find(|candidate| candidate.member().label == "customer_id")
                            .unwrap()
                            .projection_address()
                            .clone(),
                    ),
                    input: crate::node_system::document::PortAddressDto::from(
                        PortAddress::declared(consumer_id, PortKey::new("data").unwrap()),
                    ),
                    order: None,
                },
            ),
        )
        .unwrap_err();
    assert!(
        stale
            .to_string()
            .contains("projected connection endpoint is stale or unavailable")
    );

    state
        .with_database_writer(
            &project_instance_id,
            "main",
            ResourceRevision::INITIAL,
            OperationId::from_uuid(uuid::Uuid::from_u128(0x405)),
            |database, _| database.delete_column("customer_id"),
        )
        .unwrap();
    let projection = state
        .graph_projection_for_project(&project_instance_id, &graph_path(), "en-US")
        .unwrap();
    let document = &state.get_data().unwrap().graphs[&graph_path()].document;
    let binding = document.port_bindings.get(&materialized).unwrap();
    assert!(matches!(
        binding,
        crate::node_system::document::DynamicPortBinding::Resolved { last_known, .. }
            if last_known == &expected_metadata
    ));
    assert!(
        document
            .connections
            .values()
            .any(|connection| connection.output == materialized)
    );
    let orphan = projection
        .nodes
        .iter()
        .flat_map(|node| node.ports.iter())
        .find(|port| {
            port.address == crate::node_system::analysis::PortAddressDto::from(&materialized)
        })
        .unwrap();
    assert!(orphan.orphan);
    assert_eq!(
        orphan.display.instance_label.as_deref(),
        Some("customer_id")
    );
    assert_eq!(
        orphan
            .resolved_type
            .as_ref()
            .and_then(|resolved| resolved.data_type.clone()),
        Some(crate::graph::DataType::DataSeries(Box::new(
            crate::graph::DataType::Int64,
        ))),
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn dataframe_decompose_preserves_missing_database_diagnostic() {
    let (state, root, _, _) = dataframe_decompose_production_fixture(false);
    let source = state.capture_projection_source(&graph_path()).unwrap();
    let (analysis, plan) = state
        .get_or_compile_current_from_source(&graph_path(), &source)
        .unwrap();
    let diagnostic_codes = analysis
        .payload
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    let resource_diagnostic = analysis
        .payload
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.resource.resolution_failed")
        .unwrap();

    assert!(diagnostic_codes.contains(&"compiler.resource.resolution_failed"));
    assert!(!diagnostic_codes.contains(&"compiler.interface.resolver_missing"));
    assert!(!diagnostic_codes.contains(&"compiler.interface.resolver_failed"));
    assert!(plan.is_none());
    assert_eq!(
        resource_diagnostic
            .arguments
            .get("resource_key")
            .map(Box::as_ref),
        Some("databases/main")
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn projection_title<'a>(
    projection: &'a crate::node_system::analysis::EditorGraphProjectionDto,
    node_type: &str,
) -> &'a str {
    projection
        .nodes
        .iter()
        .find(|node| node.node_type_id.as_ref() == node_type)
        .unwrap()
        .display
        .title
        .as_ref()
}

#[test]
fn resource_rename_updates_editor_title() {
    let (state, root) = state_with_project_path("resource-title-renames");
    crate::project::fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref())
        .unwrap();
    let function_path = state
        .create_graph_resource_fixture("Calculate Sales", GraphDocumentKind::Function)
        .unwrap();
    load_graph(&state, &function_path).unwrap();
    let event_path = GraphResourcePath::new("events/Titles.yssbi-event").unwrap();
    let variable = state
        .add_variable(
            "Revenue",
            crate::graph::value::DataType::Int64,
            crate::graph::value::DataValue::Int64(1),
            "",
            crate::variable::VariableScope::Global,
            Vec::new(),
        )
        .unwrap();
    let variable_id = variable.id;
    let csv = root.join("sales.csv");
    std::fs::write(&csv, "amount\n1\n").unwrap();
    let imported = crate::application::database::load_database(
        &state,
        &state.capture_project_session().unwrap().instance_id,
        OperationId::new(),
        crate::schema::DatabaseEngineDTO::Csv {
            path: csv.to_string_lossy().into_owned(),
            delimiter: ',',
            has_header: true,
            infer_schema_length: None,
        },
    )
    .unwrap()
    .data;
    let database_id = imported.id;
    let initial_database_name = imported.name;
    let mut event = GraphResourceDocument::new("Titles", GraphDocumentKind::Event);
    for (index, node_type, parameter, resource) in [
        (
            1,
            "yssbi.project.function.call",
            "target",
            function_path.as_str().to_owned(),
        ),
        (
            2,
            "yssbi.project.variable.get",
            "variable",
            format!("variables/{variable_id}"),
        ),
        (
            3,
            "yssbi.dataframe.source.get",
            "dataframe",
            format!("databases/{database_id}"),
        ),
    ] {
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(index));
        event.document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: crate::node_system::document::NodePosition { x: 0.0, y: 0.0 },
                parameters: std::collections::BTreeMap::from([(
                    crate::node_system::protocol::ParameterKey::new(parameter).unwrap(),
                    serde_json::json!(resource),
                )]),
                user_label: None,
            },
        );
    }
    state.insert_graph(event_path.clone(), event).unwrap();

    let initial = state.graph_projection(&event_path, "en-US").unwrap();
    let initial_function_version =
        compile_resources_from_data(&state.get_data().unwrap(), Default::default())
            .unwrap()
            .versions[&crate::node_system::analysis::ResourceKey::new(function_path.as_str())]
            .clone();

    assert_eq!(
        projection_title(&initial, "yssbi.project.function.call"),
        "Calculate Sales"
    );
    assert_eq!(
        projection_title(&initial, "yssbi.project.variable.get"),
        "Revenue"
    );
    assert_eq!(
        projection_title(&initial, "yssbi.dataframe.source.get"),
        initial_database_name
    );

    let renamed_function = state
        .rename_graph_resource_fixture(
            &state.project_instance_id(),
            &function_path,
            "Calculate Margin",
        )
        .unwrap()
        .path;
    state
        .update_variable(
            &variable_id,
            Some("Net Revenue".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let database_revision = state
        .database_authority_revisions
        .read()
        .unwrap()
        .get(&database_id)
        .copied()
        .map(crate::node_system::document::ResourceRevision::new)
        .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
    crate::application::database::rename_database(
        &state,
        &state.capture_project_session().unwrap().instance_id,
        &database_id,
        database_revision,
        "Warehouse Sales",
        OperationId::new(),
    )
    .unwrap();

    let renamed = state.graph_projection(&event_path, "en-US").unwrap();
    let renamed_function_version =
        compile_resources_from_data(&state.get_data().unwrap(), Default::default())
            .unwrap()
            .versions[&crate::node_system::analysis::ResourceKey::new(renamed_function.as_str())]
            .clone();
    assert_ne!(renamed_function_version, initial_function_version);
    assert_eq!(
        projection_title(&renamed, "yssbi.project.function.call"),
        "Calculate Margin"
    );
    assert_eq!(
        projection_title(&renamed, "yssbi.project.variable.get"),
        "Net Revenue"
    );
    assert_eq!(
        projection_title(&renamed, "yssbi.dataframe.source.get"),
        "Warehouse Sales"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn database_schema_resolver_attaches_canonical_field_lineage() {
    let declaration = crate::database::DatabaseDecl {
        id: "main".into(),
        engine: crate::database::DatabaseEngine::InMemory {
            name: "main".into(),
        },
        schema_version: 1,
        required: true,
        name: "Main".into(),
    };
    let mut data = ProjectData::new();
    data.databases.insert("main".into(), declaration);
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let database_schemas = std::collections::BTreeMap::from([(
        resource,
        vec![crate::schema::ColumnInfoDTO {
            name: "value".into(),
            dtype: "String".into(),
        }],
    )]);
    let resources = compile_resources_from_data(&data, database_schemas).unwrap();
    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let mut graph = crate::node_system::document::GraphDocument::default();
    let mut source = node("yssbi.dataframe.source.get");
    source.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    );
    let output = PortAddress::declared(source.id, PortKey::new("dataframe").unwrap());
    graph.nodes.insert(source.id, source);

    let result = crate::node_system::compiler::GraphCompiler::with_schema_resolvers(
        &registry,
        &resources,
        resources.schema_resolvers(),
    )
    .compile(&graph);

    assert_eq!(
        result.analysis.resolved_schemas[&output].fields[0].lineage,
        Some(crate::node_system::protocol::SchemaFieldLineage {
            source: "databases/main".into(),
            field: "value".into(),
        })
    );
}

#[test]
fn database_resource_version_changes_with_resolved_column_type() {
    let declaration = crate::database::DatabaseDecl {
        id: "main".into(),
        engine: crate::database::DatabaseEngine::InMemory {
            name: "main".into(),
        },
        schema_version: 1,
        required: true,
        name: "Main".into(),
    };
    let mut data = ProjectData::new();
    data.databases.insert("main".into(), declaration);
    let resource = crate::node_system::plan::ResourceId::new("databases/main").unwrap();
    let key = crate::node_system::analysis::ResourceKey::new("databases/main");
    let schema = |dtype: &str| {
        std::collections::BTreeMap::from([(
            resource.clone(),
            vec![crate::schema::ColumnInfoDTO {
                name: "value".into(),
                dtype: dtype.into(),
            }],
        )])
    };

    let int_version = compile_resources_from_data(&data, schema("Int64"))
        .unwrap()
        .versions[&key]
        .clone();
    let string_version = compile_resources_from_data(&data, schema("String"))
        .unwrap()
        .versions[&key]
        .clone();

    assert_ne!(int_version, string_version);
}
