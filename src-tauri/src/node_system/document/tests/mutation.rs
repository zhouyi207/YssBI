use super::*;

#[test]
fn parameterized_static_creation_is_editable_with_empty_parameters() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
            required_parameters: Box::new([ParameterKey::new("columns").unwrap()]),
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
        connect_from: None,
    }
    .into_patch(
        &graph_path("events/parameterized"),
        &GraphDocument::default(),
        &registry,
    )
    .unwrap();

    let GraphDocumentOperation::InsertNode { node } = &patch.operations[0] else {
        panic!("parameterized creation must insert a node");
    };
    assert_eq!(node.node_type.as_str(), "yssbi.dataframe.project");
    assert!(node.parameters.is_empty());
}

#[test]
fn parameterized_static_missing_parameter_remains_compile_blocking() {
    struct EmptyResources;
    impl ResourceSnapshot for EmptyResources {
        fn versions(&self) -> ResourceVersionSet {
            ResourceVersionSet::new()
        }
    }

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = node_id(989);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();

    let compiled = GraphCompiler::new(&registry, &EmptyResources).compile(&document);

    assert!(compiled.semantic.is_none());
    assert!(compiled.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.parameter.required"
            && matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Parameter {
                    node_id: diagnostic_node,
                    key,
                } if *diagnostic_node == node_id && key.as_str() == "columns"
            )
    }));
}

#[test]
fn forged_parameterized_static_descriptors_have_zero_effects() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let document = GraphDocument::default();
    let project = NodeTypeId::new("yssbi.dataframe.project").unwrap();
    let filter = NodeTypeId::new("yssbi.dataframe.filter.rows").unwrap();
    let columns = ParameterKey::new("columns").unwrap();
    let predicate = ParameterKey::new("predicate").unwrap();
    let descriptors = [
        crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: project.clone(),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project.clone(),
            required_parameters: Box::new([]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project.clone(),
            required_parameters: Box::new([columns.clone(), predicate.clone()]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project.clone(),
            required_parameters: Box::new([columns.clone(), columns.clone()]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: project,
            required_parameters: Box::new([predicate]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: filter,
            required_parameters: Box::new([columns]),
        },
        crate::node_system::catalog::NodeCreationDescriptor::ParameterizedStatic {
            node_type_id: NodeTypeId::new("yssbi.numeric.add.int64").unwrap(),
            required_parameters: Box::new([]),
        },
    ];

    for descriptor in descriptors {
        let result = EditorGraphMutationDto::CreateNode {
            descriptor,
            position: NodePosition { x: 1.0, y: 2.0 },
            user_label: None,
            connect_from: None,
        }
        .into_patch(&graph_path("events/forged"), &document, &registry);
        assert!(result.is_err());
        assert!(document.nodes.is_empty());
    }
}

#[test]
fn set_parameters_atomically_replaces_and_validates_the_complete_map() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = node_id(990);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let parameters = ParameterValues::from([(
        ParameterKey::new("columns").unwrap(),
        json!(["status", "amount"]),
    )]);
    let mutation = EditorGraphMutationDto::SetParameters {
        node_id,
        parameters: parameters.clone(),
    };
    assert_eq!(
        serde_json::to_value(&mutation).unwrap(),
        json!({
            "type": "setParameters",
            "payload": {
                "nodeId": node_id,
                "parameters": { "columns": ["status", "amount"] }
            }
        }),
    );

    let patch = mutation
        .into_patch(&graph_path("events/parameters"), &document, &registry)
        .unwrap();
    assert_eq!(patch.operations.len(), 1);
    let GraphDocumentOperation::UpdateNode { before, after } = &patch.operations[0] else {
        panic!("parameter update must be one node replacement");
    };
    assert!(before.parameters.is_empty());
    assert_eq!(after.parameters, parameters);
}

#[test]
fn invalid_atomic_parameter_mutations_have_zero_effects() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = node_id(991);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.dataframe.filter.rows").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    for parameters in [
        ParameterValues::new(),
        ParameterValues::from([(
            ParameterKey::new("predicate").unwrap(),
            json!({
                "column": "count",
                "operator": "greaterThan",
                "value": { "type": "integer", "value": 9007199254740993_i64 }
            }),
        )]),
        ParameterValues::from([(ParameterKey::new("columns").unwrap(), json!(["forged"]))]),
    ] {
        let result = EditorGraphMutationDto::SetParameters {
            node_id,
            parameters,
        }
        .into_patch(&graph_path("events/parameters"), &document, &registry);
        assert!(result.is_err());
        assert!(document.nodes[&node_id].parameters.is_empty());
    }
}

#[test]
fn create_node_rejects_protocol_scope_mismatch() {
    let registry = editor_mutation_registry_with(NodeScope::Event, 0);
    let mutation = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
        connect_from: None,
    };

    let error = mutation
        .into_patch(
            &graph_path("functions/scope-mismatch"),
            &GraphDocument::default(),
            &registry,
        )
        .unwrap_err();

    assert!(error.to_string().contains("scope"));
}

#[test]
fn create_node_materializes_required_user_created_ports() {
    let registry = editor_mutation_registry_with(NodeScope::Any, 2);
    let patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        position: NodePosition { x: 1.0, y: 2.0 },
        user_label: None,
        connect_from: None,
    }
    .into_patch(
        &graph_path("events/initial-ports"),
        &GraphDocument::default(),
        &registry,
    )
    .unwrap();

    let node_id = match &patch.operations[0] {
        GraphDocumentOperation::InsertNode { node } => node.id,
        operation => panic!("expected node insertion first, got {operation:?}"),
    };
    let bindings = patch
        .operations
        .iter()
        .skip(1)
        .map(|operation| match operation {
            GraphDocumentOperation::InsertPortBinding {
                address,
                binding: DynamicPortBinding::UserCreated { .. },
            } => address,
            operation => panic!("unexpected create operation: {operation:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    assert!(bindings.iter().all(|address| address.node_id == node_id));
    assert_ne!(bindings[0], bindings[1]);
}

#[test]
fn builtin_loop_create_materializes_one_complete_carried_member() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut parameters = ParameterValues::new();
    parameters.insert(ParameterKey::new("max_iterations").unwrap(), json!(100));
    let node_type_id = NodeTypeId::new("yssbi.control.loop").unwrap();
    let protocol = registry.protocol(&node_type_id).unwrap();
    validate_parameters(protocol, &parameters).unwrap();
    let patch = GraphDocumentPatch::new(create_node_operations(
        protocol,
        node_type_id,
        NodePosition { x: 1.0, y: 2.0 },
        parameters,
        None,
    ));

    let addresses = grouped_binding_addresses(&patch);
    assert_eq!(addresses.len(), 4);
    assert_eq!(
        addresses
            .iter()
            .map(|address| instance_template(address))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["body_input", "initial_source", "next_source", "result"])
    );
    assert_eq!(
        addresses
            .iter()
            .map(instance_identity)
            .collect::<BTreeSet<_>>()
            .len(),
        1,
        "one carried member must share one identity across all templates"
    );
}

#[test]
fn builtin_branch_adds_complete_members_with_stable_shared_identities() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/grouped-branch");
    let owner = node_id(905);
    let templates = ["then_source", "else_source", "result"];

    for requested in templates {
        let mut document = GraphDocument::default();
        document
            .create_node(builtin_control_node(owner, "yssbi.control.branch"))
            .unwrap();
        let patch = EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: PortKey::new(requested).unwrap(),
            order: Some(OrderKey("member".into())),
        }
        .into_patch(&path, &document, &registry)
        .unwrap();
        let addresses = grouped_binding_addresses(&patch);
        assert_eq!(addresses.len(), 3);
        assert_eq!(
            addresses
                .iter()
                .map(|address| instance_template(address))
                .collect::<BTreeSet<_>>(),
            templates.into_iter().collect()
        );
        assert_eq!(
            addresses
                .iter()
                .map(instance_identity)
                .collect::<BTreeSet<_>>()
                .len(),
            1
        );
    }

    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.branch"))
        .unwrap();
    let first = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: PortKey::new("result").unwrap(),
        order: Some(OrderKey("z".into())),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let first_id = instance_identity(&grouped_binding_addresses(&first)[0]);
    let mut reversed = first.operations.to_vec();
    reversed.reverse();
    document
        .apply_patch(&GraphDocumentPatch::new(reversed))
        .unwrap();

    let second = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: PortKey::new("then_source").unwrap(),
        order: Some(OrderKey("a".into())),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let second_id = instance_identity(&grouped_binding_addresses(&second)[0]);
    assert_ne!(first_id, second_id);
    document.apply_patch(&second).unwrap();

    let mut by_identity = BTreeMap::<PortInstanceId, BTreeSet<&str>>::new();
    for address in document.port_bindings.keys() {
        by_identity
            .entry(instance_identity(address))
            .or_default()
            .insert(instance_template(address));
    }
    assert_eq!(by_identity.len(), 2);
    assert!(
        by_identity
            .values()
            .all(|members| { members == &templates.into_iter().collect::<BTreeSet<_>>() })
    );
}

#[test]
fn removing_any_group_member_atomically_removes_the_complete_member() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/grouped-remove");
    let owner = node_id(906);
    let source = node_id(907);
    let sink = node_id(908);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.branch"))
        .unwrap();
    document.create_node(node(source)).unwrap();
    document.create_node(node(sink)).unwrap();

    for order in ["first", "second"] {
        let patch = EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: PortKey::new("else_source").unwrap(),
            order: Some(OrderKey(order.into())),
        }
        .into_patch(&path, &document, &registry)
        .unwrap();
        document.apply_patch(&patch).unwrap();
    }
    let removed_id = document
        .port_bindings
        .keys()
        .map(instance_identity)
        .min()
        .unwrap();
    let grouped =
        |template| PortAddress::instance(owner, PortKey::new(template).unwrap(), removed_id);
    let then_source = grouped("then_source");
    let else_source = grouped("else_source");
    let result = grouped("result");
    document
        .connect(declared(source, "output"), then_source.clone(), None)
        .unwrap();
    document
        .connect(declared(source, "output_2"), else_source.clone(), None)
        .unwrap();
    document
        .connect(result.clone(), declared(sink, "input"), None)
        .unwrap();
    document
        .set_literal(then_source.clone(), Some(json!(1)))
        .unwrap();
    document
        .set_literal(else_source.clone(), Some(json!(2)))
        .unwrap();
    let before = document.clone();

    let patch = EditorGraphMutationDto::RemovePortInstance {
        address: else_source.into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&patch).unwrap();

    assert!(document.port_bindings.keys().all(|address| {
        !matches!(&address.port, PortRef::Instance { instance_id, .. } if *instance_id == removed_id)
    }));
    assert!(document.input_states.keys().all(|address| {
        !matches!(&address.port, PortRef::Instance { instance_id, .. } if *instance_id == removed_id)
    }));
    assert!(document.connections.values().all(|connection| {
        instance_identity_if_present(&connection.output) != Some(removed_id)
            && instance_identity_if_present(&connection.input) != Some(removed_id)
    }));
    assert_eq!(document.port_bindings.len(), 3);

    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

#[test]
fn loop_partial_member_does_not_inflate_complete_count_or_block_repair() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/partial-loop");
    let owner = node_id(909);
    let complete_id = instance_id(910);
    let partial_id = instance_id(911);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.loop"))
        .unwrap();
    for template in ["initial_source", "body_input", "next_source", "result"] {
        bind_user_port(&mut document, owner, template, complete_id);
    }
    bind_user_port(&mut document, owner, "initial_source", partial_id);

    assert!(
        EditorGraphMutationDto::RemovePortInstance {
            address: PortAddress::instance(owner, PortKey::new("result").unwrap(), complete_id,)
                .into(),
        }
        .into_patch(&path, &document, &registry)
        .is_err(),
        "the only complete member must satisfy Loop min=1"
    );

    let remove_partial = EditorGraphMutationDto::RemovePortInstance {
        address: PortAddress::instance(owner, PortKey::new("initial_source").unwrap(), partial_id)
            .into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&remove_partial).unwrap();
    assert!(
        document
            .port_bindings
            .keys()
            .all(|address| { instance_identity_if_present(address) != Some(partial_id) })
    );
    assert_eq!(
        document
            .port_bindings
            .keys()
            .filter(|address| instance_identity_if_present(address) == Some(complete_id))
            .count(),
        4
    );
}

#[test]
fn loop_with_only_a_partial_member_can_remove_it_below_group_minimum() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/partial-only-loop");
    let owner = node_id(912);
    let partial_id = instance_id(913);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.loop"))
        .unwrap();
    bind_user_port(&mut document, owner, "next_source", partial_id);

    let patch = EditorGraphMutationDto::RemovePortInstance {
        address: PortAddress::instance(owner, PortKey::new("next_source").unwrap(), partial_id)
            .into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&patch).unwrap();
    assert!(document.port_bindings.is_empty());
}

#[test]
fn partial_member_does_not_consume_group_maximum() {
    let registry = builtin_registry_with_branch_group_max(1);
    let path = graph_path("events/partial-max");
    let owner = node_id(914);
    let partial_id = instance_id(915);
    let mut document = GraphDocument::default();
    document
        .create_node(builtin_control_node(owner, "yssbi.control.branch"))
        .unwrap();
    bind_user_port(&mut document, owner, "then_source", partial_id);

    let complete = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: PortKey::new("result").unwrap(),
        order: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&complete).unwrap();
    assert!(
        EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: PortKey::new("else_source").unwrap(),
            order: None,
        }
        .into_patch(&path, &document, &registry)
        .is_err(),
        "the newly added complete member must consume max=1"
    );
}

#[test]
fn create_node_with_connect_from_builds_one_atomic_patch_in_both_directions() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/atomic-create-connect");
    let snapshot = compatibility_snapshot();

    let output_node = node_id(1200);
    let mut output_document = GraphDocument::default();
    output_document
        .create_node(DocumentNode {
            id: output_node,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let output = declared(output_node, "value");
    let output_source = crate::node_system::compatibility::SourcePort {
        address: output.clone(),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        type_parameters: Box::new([]),
    };
    let output_validation = create_connect_validation_snapshot(
        &output_document,
        output.clone(),
        PortDirection::Output,
        EditorMutationPortType::Ready {
            expression: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            type_parameters: Box::new([]),
        },
    );
    let output_patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.numeric.add.int64").unwrap(),
        },
        position: NodePosition { x: 10.0, y: 20.0 },
        user_label: None,
        connect_from: Some(output.clone().into()),
    }
    .into_patch_with_editor_validation(
        &path,
        &output_document,
        &registry,
        Some(&snapshot),
        Some(&output_source),
        Some(&output_validation),
    )
    .unwrap();
    assert!(matches!(
        output_patch.operations.last(),
        Some(GraphDocumentOperation::InsertConnection { connection }) if connection.output == output
    ));

    let input_node = node_id(1201);
    let mut input_document = GraphDocument::default();
    input_document
        .create_node(DocumentNode {
            id: input_node,
            node_type: NodeTypeId::new("yssbi.numeric.add.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let input = declared(input_node, "left");
    let input_source = crate::node_system::compatibility::SourcePort {
        address: input.clone(),
        direction: PortDirection::Input,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        type_parameters: Box::new([]),
    };
    let input_validation = create_connect_validation_snapshot(
        &input_document,
        input.clone(),
        PortDirection::Input,
        EditorMutationPortType::Ready {
            expression: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            type_parameters: Box::new([]),
        },
    );
    let input_patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
        },
        position: NodePosition { x: 10.0, y: 20.0 },
        user_label: None,
        connect_from: Some(input.clone().into()),
    }
    .into_patch_with_editor_validation(
        &path,
        &input_document,
        &registry,
        Some(&snapshot),
        Some(&input_source),
        Some(&input_validation),
    )
    .unwrap();
    assert!(matches!(
        input_patch.operations.last(),
        Some(GraphDocumentOperation::InsertConnection { connection }) if connection.input == input
    ));
}

#[test]
fn phase1_connection_capability_create_and_connect_replaces_occupied_single_source() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/atomic-create-replace");
    let snapshot = compatibility_snapshot();
    let input_node = node_id(1210);
    let incumbent_node = node_id(1211);
    let incumbent_id = connection_id(1212);
    let input = declared(input_node, "left");
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: input_node,
            node_type: NodeTypeId::new("yssbi.numeric.add.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    document
        .create_node(DocumentNode {
            id: incumbent_node,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: -10.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    document.connections.insert(
        incumbent_id,
        DocumentConnection {
            id: incumbent_id,
            output: declared(incumbent_node, "value"),
            input: input.clone(),
            order: None,
        },
    );
    let source = crate::node_system::compatibility::SourcePort {
        address: input.clone(),
        direction: PortDirection::Input,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        type_parameters: Box::new([]),
    };

    let validation = create_connect_validation_snapshot(
        &document,
        input.clone(),
        PortDirection::Input,
        EditorMutationPortType::Ready {
            expression: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            type_parameters: Box::new([]),
        },
    );
    let patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
        },
        position: NodePosition { x: 10.0, y: 20.0 },
        user_label: None,
        connect_from: Some(input.clone().into()),
    }
    .into_patch_with_editor_validation(
        &path,
        &document,
        &registry,
        Some(&snapshot),
        Some(&source),
        Some(&validation),
    )
    .unwrap();

    assert!(patch.operations.iter().any(|operation| matches!(
        operation,
        GraphDocumentOperation::RemoveConnection { connection }
            if connection.id == incumbent_id
    )));
    assert!(matches!(
        patch.operations.last(),
        Some(GraphDocumentOperation::InsertConnection { connection }) if connection.input == input
    ));
    let mut committed = document;
    committed.apply_patch(&patch).unwrap();
    assert!(!committed.connections.contains_key(&incumbent_id));
    assert_eq!(
        committed
            .connections
            .values()
            .filter(|connection| connection.input == input)
            .count(),
        1
    );
}

#[test]
fn phase1_connection_capability_create_and_connect_rejects_untrusted_source_types_before_replacement()
 {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/atomic-create-replace-type-errors");
    let catalog = compatibility_snapshot();
    let input_node = node_id(1220);
    let incumbent_node = node_id(1221);
    let input = declared(input_node, "left");
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: input_node,
            node_type: NodeTypeId::new("yssbi.numeric.add.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    document
        .create_node(DocumentNode {
            id: incumbent_node,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: -10.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let incumbent_id = connection_id(1222);
    document.connections.insert(
        incumbent_id,
        DocumentConnection {
            id: incumbent_id,
            output: declared(incumbent_node, "value"),
            input: input.clone(),
            order: None,
        },
    );
    let best_effort_source = crate::node_system::compatibility::SourcePort {
        address: input.clone(),
        direction: PortDirection::Input,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        type_parameters: Box::new([]),
    };
    let cases = [
        (
            EditorMutationPortType::Ready {
                expression: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
                type_parameters: Box::new([]),
            },
            EditorMutationErrorCode::GraphConnectionTypeMismatch,
        ),
        (
            EditorMutationPortType::MissingInternalTypeExpr,
            EditorMutationErrorCode::GraphConnectionTypeUnavailable,
        ),
        (
            EditorMutationPortType::Unresolved {
                expression: TypeExpr::Generic(TypeParameterId::new("missing").unwrap()),
                type_parameters: Box::new([]),
            },
            EditorMutationErrorCode::GraphConnectionTypeUnresolved,
        ),
    ];

    for (port_type, expected) in cases {
        let before = document.clone();
        let validation = create_connect_validation_snapshot(
            &document,
            input.clone(),
            PortDirection::Input,
            port_type,
        );
        let error = EditorGraphMutationDto::CreateNode {
            descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
                node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            },
            position: NodePosition { x: 10.0, y: 20.0 },
            user_label: None,
            connect_from: Some(input.clone().into()),
        }
        .into_patch_with_editor_validation(
            &path,
            &document,
            &registry,
            Some(&catalog),
            Some(&best_effort_source),
            Some(&validation),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MutationConflict::Editor(EditorMutationError { code, .. }) if code == expected
        ));
        assert_graph_content_eq(&document, &before);
        assert!(document.connections.contains_key(&incumbent_id));
    }
}

#[test]
fn incompatible_atomic_create_is_rejected_without_document_effects() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = graph_path("events/incompatible-create-connect");
    let snapshot = compatibility_snapshot();
    let source_node = node_id(1202);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: source_node,
            node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        })
        .unwrap();
    let before = document.clone();
    let address = declared(source_node, "value");
    let source = crate::node_system::compatibility::SourcePort {
        address: address.clone(),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        type_parameters: Box::new([]),
    };

    let validation = create_connect_validation_snapshot(
        &document,
        address.clone(),
        PortDirection::Output,
        EditorMutationPortType::Ready {
            expression: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            type_parameters: Box::new([]),
        },
    );
    let error = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.logic.not").unwrap(),
        },
        position: NodePosition { x: 10.0, y: 20.0 },
        user_label: None,
        connect_from: Some(address.into()),
    }
    .into_patch_with_editor_validation(
        &path,
        &document,
        &registry,
        Some(&snapshot),
        Some(&source),
        Some(&validation),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        MutationConflict::Editor(EditorMutationError {
            code: EditorMutationErrorCode::GraphConnectionTypeMismatch,
            ..
        })
    ));
    assert_graph_content_eq(&document, &before);
}

#[test]
fn editor_connect_materializes_a_projected_input_on_the_input_side() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/projected-editor-input");
    let source_id = node_id(1203);
    let target_id = node_id(1204);
    let mut document = GraphDocument::default();
    document
        .create_node(editor_mutation_node(source_id))
        .unwrap();
    document
        .create_node(editor_mutation_node(target_id))
        .unwrap();
    let projected = PortAddress::instance(
        target_id,
        PortKey::new("inputs").unwrap(),
        PortInstanceId::from_uuid(Uuid::from_u128(1205)),
    );
    let member = projected_member(path.0.as_ref(), document.revision, target_id);
    let plan = super::ProjectedConnectPlan {
        projection_address: projected.clone(),
        direction: PortDirection::Input,
        kind: PortKind::Data,
        connections: ConnectionsPerPort::Single,
        authorization: authorization(member.clone()),
        member,
    };

    let patch = EditorGraphMutationDto::Connect {
        output: declared(source_id, "output").into(),
        input: projected.into(),
        order: None,
    }
    .into_patch_with_compatibility(&path, &document, &registry, None, None, Some(plan))
    .unwrap();

    let materialized = match &patch.operations[0] {
        GraphDocumentOperation::InsertPortBinding { address, .. } => address,
        operation => panic!("expected binding insertion, got {operation:?}"),
    };
    assert!(matches!(
        &patch.operations[1],
        GraphDocumentOperation::InsertConnection { connection }
            if connection.output == declared(source_id, "output")
                && connection.input == *materialized
    ));
}

#[test]
fn create_connect_and_add_port_allocate_identity_in_rust() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/editor-mutation");
    let existing_id = node_id(911);
    let mut document = GraphDocument::default();
    document
        .create_node(editor_mutation_node(existing_id))
        .unwrap();

    let create_patch = EditorGraphMutationDto::CreateNode {
        descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
            node_type_id: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        },
        position: NodePosition { x: 5.0, y: 8.0 },
        user_label: None,
        connect_from: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let created_id = match &create_patch.operations[0] {
        GraphDocumentOperation::InsertNode { node } => node.id,
        operation => panic!("expected node insertion first, got {operation:?}"),
    };
    assert_ne!(created_id, existing_id);
    assert!(matches!(
        &create_patch.operations[1..],
        [GraphDocumentOperation::InsertPortBinding {
            address,
            binding: DynamicPortBinding::UserCreated { .. },
        }] if address.node_id == created_id
    ));
    document.apply_patch(&create_patch).unwrap();

    let add_patch = EditorGraphMutationDto::AddPortInstance {
        node_id: existing_id,
        template: PortKey::new("inputs").unwrap(),
        order: None,
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    let input = match &add_patch.operations[..] {
        [
            GraphDocumentOperation::InsertPortBinding {
                address,
                binding: DynamicPortBinding::UserCreated { .. },
            },
        ] => address.clone(),
        operations => panic!("unexpected add-port operations: {operations:?}"),
    };
    assert!(matches!(input.port, PortRef::Instance { .. }));
    document.apply_patch(&add_patch).unwrap();

    let output = declared(created_id, "output");
    let type_parameters = registry
        .protocol(&document.nodes[&created_id].node_type)
        .unwrap()
        .interface
        .type_parameters
        .clone();
    let ready_int64 = || crate::node_system::compatibility::EditorMutationPortType::Ready {
        expression: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        type_parameters: type_parameters.clone(),
    };
    let validation = crate::node_system::compatibility::EditorMutationValidationSnapshot {
        graph_revision: document.revision,
        ports: BTreeMap::from([
            (
                output.clone(),
                crate::node_system::compatibility::EditorMutationPortValidation {
                    direction: PortDirection::Output,
                    kind: PortKind::Data,
                    orphan: false,
                    port_type: ready_int64(),
                },
            ),
            (
                input.clone(),
                crate::node_system::compatibility::EditorMutationPortValidation {
                    direction: PortDirection::Input,
                    kind: PortKind::Data,
                    orphan: false,
                    port_type: ready_int64(),
                },
            ),
        ]),
    };
    let connect_patch = EditorGraphMutationDto::Connect {
        output: output.into(),
        input: input.clone().into(),
        order: None,
    }
    .into_patch_with_editor_validation(&path, &document, &registry, None, None, Some(&validation))
    .unwrap();
    let allocated_connection = match &connect_patch.operations[..] {
        [GraphDocumentOperation::InsertConnection { connection }] => connection,
        operations => panic!("unexpected connect operations: {operations:?}"),
    };
    assert_eq!(allocated_connection.output.node_id, created_id);
    assert_eq!(allocated_connection.input, input);
    assert!(!document.connections.contains_key(&allocated_connection.id));
}

#[test]
fn move_nodes_is_atomic_and_reversible() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/move-nodes");
    let first = node_id(921);
    let second = node_id(922);
    let missing = node_id(923);
    let mut document = GraphDocument::default();
    document.create_node(editor_mutation_node(first)).unwrap();
    document.create_node(editor_mutation_node(second)).unwrap();
    let before = document.clone();

    let invalid = EditorGraphMutationDto::MoveNodes {
        positions: vec![
            NodePositionMutationDto {
                node_id: first,
                position: NodePosition { x: 13.0, y: 21.0 },
            },
            NodePositionMutationDto {
                node_id: missing,
                position: NodePosition { x: 34.0, y: 55.0 },
            },
        ],
    };
    assert!(invalid.into_patch(&path, &document, &registry).is_err());
    assert_graph_content_eq(&document, &before);

    let patch = EditorGraphMutationDto::MoveNodes {
        positions: vec![
            NodePositionMutationDto {
                node_id: first,
                position: NodePosition { x: 13.0, y: 21.0 },
            },
            NodePositionMutationDto {
                node_id: second,
                position: NodePosition { x: 34.0, y: 55.0 },
            },
        ],
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    assert_eq!(patch.operations.len(), 2);

    document.apply_patch(&patch).unwrap();
    assert_eq!(
        document.nodes[&first].position,
        NodePosition { x: 13.0, y: 21.0 }
    );
    assert_eq!(
        document.nodes[&second].position,
        NodePosition { x: 34.0, y: 55.0 }
    );
    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}

#[test]
fn user_created_port_enforces_protocol_min_and_max() {
    let registry = editor_mutation_registry();
    let path = graph_path("events/user-created-port");
    let owner = node_id(931);
    let template = PortKey::new("inputs").unwrap();
    let first = PortAddress::instance(owner, template.clone(), instance_id(932));
    let mut document = GraphDocument::default();
    document.create_node(editor_mutation_node(owner)).unwrap();
    document
        .bind_port(
            first.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("a".into()),
            },
        )
        .unwrap();

    let add_patch = EditorGraphMutationDto::AddPortInstance {
        node_id: owner,
        template: template.clone(),
        order: Some(OrderKey("b".into())),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&add_patch).unwrap();
    assert!(
        EditorGraphMutationDto::AddPortInstance {
            node_id: owner,
            template: template.clone(),
            order: None,
        }
        .into_patch(&path, &document, &registry)
        .is_err()
    );

    let second = document
        .port_bindings
        .keys()
        .find(|address| **address != first)
        .cloned()
        .unwrap();
    let remove_patch = EditorGraphMutationDto::RemovePortInstance {
        address: second.into(),
    }
    .into_patch(&path, &document, &registry)
    .unwrap();
    document.apply_patch(&remove_patch).unwrap();
    assert!(
        EditorGraphMutationDto::RemovePortInstance {
            address: first.into(),
        }
        .into_patch(&path, &document, &registry)
        .is_err()
    );
}

#[test]
fn declared_port_address_needs_no_persisted_instance() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();

    document
        .connect(declared(first, "output"), declared(second, "input"), None)
        .unwrap();

    assert!(document.port_bindings.is_empty());
    assert!(document.validate().is_ok());
}

#[test]
fn instance_address_requires_a_binding() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = PortAddress::instance(second, PortKey::new("fields").unwrap(), instance_id(10));

    assert!(matches!(
        document.connect(declared(first, "output"), input.clone(), None),
        Err(DocumentError::MissingPortBinding(address)) if address == input
    ));
    assert!(document.connections.is_empty());

    document.bind_port(input.clone(), binding()).unwrap();
    document
        .connect(declared(first, "output"), input, None)
        .unwrap();
}

#[test]
fn deleting_a_node_atomically_removes_owned_and_incident_data() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = PortAddress::instance(second, PortKey::new("fields").unwrap(), instance_id(10));
    document.bind_port(input.clone(), binding()).unwrap();
    document
        .set_literal(input.clone(), Some(json!(42)))
        .unwrap();
    document
        .connect(declared(first, "output"), input, None)
        .unwrap();

    document.delete_node(second).unwrap();

    assert!(!document.nodes.contains_key(&second));
    assert!(document.connections.is_empty());
    assert!(document.port_bindings.is_empty());
    assert!(document.input_states.is_empty());
    assert!(document.validate().is_ok());
}

#[test]
fn connections_override_but_do_not_discard_literals() {
    let first = node_id(1);
    let second = node_id(2);
    let mut document = GraphDocument::default();
    document.create_node(node(first)).unwrap();
    document.create_node(node(second)).unwrap();
    let input = declared(second, "input");
    document
        .set_literal(input.clone(), Some(json!(42)))
        .unwrap();
    let connection = document
        .connect(declared(first, "output"), input.clone(), None)
        .unwrap();

    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::Connections(vec![connection])
    );
    document.disconnect(connection).unwrap();
    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::Literal(json!(42))
    );
    document.set_literal(input.clone(), None).unwrap();
    assert_eq!(
        document.effective_input_binding(&input, Some(json!(0))),
        EffectiveInputBinding::ProtocolDefault(json!(0))
    );
}

#[test]
fn mutation_rejects_wrong_resource_without_changing_the_graph() {
    let path = graph_path("events/main");
    let requested = ResourceKey::Graph(graph_path("events/other"));
    let mut store = RevisionedGraphStore::new(path.clone(), GraphDocument::default());
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        requested.clone(),
        ResourceRevision::INITIAL,
        operation_id(500),
        GraphMutation::CreateNode {
            node: node(node_id(501)),
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::ResourceMismatch { requested: actual, store: expected })
            if actual == requested && expected == ResourceKey::Graph(path)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn mutation_rejects_stale_revision_without_changing_the_graph() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let mut store = RevisionedGraphStore::new(path, GraphDocument::default());
    store
        .apply_mutation(MutationRequest::new(
            resource.clone(),
            ResourceRevision::INITIAL,
            operation_id(502),
            GraphMutation::CreateNode {
                node: node(node_id(503)),
            },
        ))
        .unwrap();
    let before = store.document().clone();

    let result = store.apply_mutation(MutationRequest::new(
        resource,
        ResourceRevision::INITIAL,
        operation_id(504),
        GraphMutation::CreateNode {
            node: node(node_id(505)),
        },
    ));

    assert!(matches!(
        result,
        Err(MutationConflict::StaleRevision {
            base_revision,
            current_revision,
        }) if base_revision == ResourceRevision::INITIAL
            && current_revision == ResourceRevision::new(1)
    ));
    assert_eq!(store.document(), &before);
}

#[test]
fn mutation_events_use_the_complete_graph_envelope() {
    let path = graph_path("events/main");
    let resource = ResourceKey::Graph(path.clone());
    let operation = operation_id(510);
    let mut store = RevisionedGraphStore::new(path.clone(), GraphDocument::default());

    let event = store
        .apply_mutation(MutationRequest::new(
            resource,
            ResourceRevision::INITIAL,
            operation,
            GraphMutation::CreateNode {
                node: node(node_id(511)),
            },
        ))
        .unwrap();

    assert_eq!(event.graph_path, path);
    assert_eq!(event.from_revision, ResourceRevision::INITIAL);
    assert_eq!(event.to_revision, ResourceRevision::new(1));
    assert_eq!(event.caused_by, Some(operation));
    assert_eq!(event.payload.operations.len(), 1);
}

#[test]
fn revision_gap_reports_the_missing_delta_range() {
    let event = GraphDeltaEvent {
        graph_path: graph_path("events/main"),
        from_revision: ResourceRevision::new(4),
        to_revision: ResourceRevision::new(5),
        caused_by: None,
        payload: GraphDocumentPatch::new(Vec::new()),
    };

    assert_eq!(
        detect_revision_gap(ResourceRevision::new(2), &event),
        Some(RevisionGap {
            expected_before_revision: ResourceRevision::new(2),
            actual_before_revision: ResourceRevision::new(4),
        })
    );
}
