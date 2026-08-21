use super::*;

#[test]
fn editor_parameter_validation_applies_registered_nominal_codec() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut protocol = registry
        .protocol(&NodeTypeId::new("yssbi.dataframe.rename").unwrap())
        .unwrap()
        .clone();
    let columns = ParameterKey::new("columns").unwrap();
    protocol.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: columns.clone(),
        title_key: I18nKey::new("parameters.columns.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Concrete(
            TypeId::new(crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID).unwrap(),
        ),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Auto,
        presentation: ParameterPresentation::DetailPanel,
    }])
    .unwrap();

    assert!(
        validate_parameters_with_registry(
            &registry,
            &protocol,
            &ParameterValues::from([(columns.clone(), serde_json::json!(["b", "a"]))]),
        )
        .is_ok()
    );
    assert!(
        validate_parameters_with_registry(
            &registry,
            &protocol,
            &ParameterValues::from([(columns, serde_json::json!([]))]),
        )
        .is_err()
    );
}

#[test]
fn editor_mutation_validates_parameter_type_and_constraints() {
    let registry = validation_registry();
    let protocol = registry
        .protocol(&NodeTypeId::new(NODE_TYPE).unwrap())
        .unwrap();
    let validate = |parameters: ParameterValues| validate_parameters(protocol, &parameters);
    let count = ParameterKey::new("count").unwrap();
    let mode = ParameterKey::new("mode").unwrap();
    let label = ParameterKey::new("label").unwrap();

    assert!(validate(ParameterValues::new()).is_err());
    assert!(
        validate(BTreeMap::from([(count.clone(), json!("two"))]))
            .unwrap_err()
            .to_string()
            .contains("declared type")
    );
    assert!(
        validate(BTreeMap::from([(count.clone(), json!(4))]))
            .unwrap_err()
            .to_string()
            .contains("constraints")
    );
    assert!(
        validate(BTreeMap::from([
            (count.clone(), json!(2)),
            (mode.clone(), json!("gamma")),
        ]))
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        validate(BTreeMap::from([
            (count.clone(), json!(2)),
            (label.clone(), json!("x")),
        ]))
        .unwrap_err()
        .to_string()
        .contains("constraints")
    );
    assert!(
        validate(BTreeMap::from([
            (count, json!(2)),
            (mode, json!("alpha")),
            (label, json!("good")),
        ]))
        .is_ok()
    );
}

#[test]
fn editor_mutation_rejects_connection_direction_and_kind_mismatches() {
    let registry = validation_registry();
    let first = node_id(1_001);
    let second = node_id(1_002);
    let document = validation_document(&[first, second]);
    let validation = validation_snapshot_for_document(&document, &registry);

    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_in").into(),
                input: declared(second, "data_in").into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("directions")
    );
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "control_in").into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("kinds")
    );
}

#[test]
fn editor_mutation_rejects_proven_incompatible_connection() {
    let registry = validation_registry();
    let first = node_id(1_003);
    let second = node_id(1_004);
    let document = validation_document(&[first, second]);

    let error = plan(
        EditorGraphMutationDto::Connect {
            output: declared(first, "string_series_out").into(),
            input: declared(second, "numeric_series_in").into(),
            order: None,
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert!(error.to_string().contains("types are incompatible"));
}

#[test]
fn editor_mutation_allows_indeterminate_connection_for_backend_analysis() {
    let registry = validation_registry();
    let first = node_id(1_005);
    let second = node_id(1_006);
    let document = validation_document(&[first, second]);

    assert!(
        plan(
            EditorGraphMutationDto::Connect {
                output: declared(first, "unknown_series_out").into(),
                input: declared(second, "numeric_series_in").into(),
                order: None,
            },
            &document,
            &registry,
        )
        .is_ok()
    );
}

#[test]
fn editor_mutation_enforces_connection_capacity_and_order_policy() {
    let registry = validation_registry();
    let first = node_id(1_011);
    let second = node_id(1_012);
    let third = node_id(1_013);
    let mut document = validation_document(&[first, second, third]);
    document
        .connect(
            declared(first, "data_out"),
            declared(second, "data_in"),
            None,
        )
        .unwrap();
    let validation = validation_snapshot_for_document(&document, &registry);

    let replacement = plan_with_validation(
        EditorGraphMutationDto::Connect {
            output: declared(third, "data_out").into(),
            input: declared(second, "data_in").into(),
            order: None,
        },
        &document,
        &registry,
        &validation,
    )
    .unwrap();
    assert!(matches!(
        replacement.operations.as_slice(),
        [
            GraphDocumentOperation::RemoveConnection { .. },
            GraphDocumentOperation::InsertConnection { .. }
        ]
    ));
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "ordered_in").into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("require an order")
    );
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(third, "data_in").into(),
                order: Some(OrderKey("a".into())),
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("cannot carry")
    );
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: declared(second, "ordered_in").into(),
                order: Some(OrderKey("a".into())),
            },
            &document,
            &registry,
            &validation,
        )
        .is_ok()
    );
}

#[test]
fn editor_mutation_rejects_orphan_and_binding_policy_mismatches() {
    let registry = validation_registry();
    let first = node_id(1_021);
    let second = node_id(1_022);
    let mut document = validation_document(&[first, second]);
    let orphan = derived_address(second, 1_023);
    document.port_bindings.insert(
        orphan.clone(),
        DynamicPortBinding::Orphan {
            origin: DynamicMemberLocator::SchemaField {
                source: SchemaSourceIdentity("source".into()),
                field: SchemaFieldIdentity("field".into()),
            },
            order: OrderKey("a".into()),
            last_known: LastKnownPortMetadata {
                label: "Field".to_owned(),
                value_type: None,
            },
        },
    );
    let validation = validation_snapshot_for_document(&document, &registry);
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: orphan.into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("orphan")
    );

    let mismatched = user_address(second, 1_024);
    document.port_bindings.insert(mismatched.clone(), binding());
    let validation = validation_snapshot_for_document(&document, &registry);
    assert!(
        plan_with_validation(
            EditorGraphMutationDto::Connect {
                output: declared(first, "data_out").into(),
                input: mismatched.into(),
                order: None,
            },
            &document,
            &registry,
            &validation,
        )
        .unwrap_err()
        .to_string()
        .contains("binding kind")
    );
}

#[test]
fn editor_mutation_enforces_literal_policy() {
    let registry = validation_registry();
    let owner = node_id(1_031);
    let document = validation_document(&[owner]);
    let literal = serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        value: Value::Integer(1),
    })
    .unwrap();

    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "data_in").into(),
                literal: Some(literal.clone()),
            },
            &document,
            &registry,
        )
        .is_ok()
    );
    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "forbidden_in").into(),
                literal: Some(literal.clone()),
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("forbids")
    );
    assert!(
        plan(
            EditorGraphMutationDto::SetLiteral {
                address: declared(owner, "data_out").into(),
                literal: Some(literal),
            },
            &document,
            &registry,
        )
        .unwrap_err()
        .to_string()
        .contains("data input")
    );
}

#[test]
fn editor_mutation_rejects_legal_typed_literal_for_the_wrong_port_type() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let owner = node_id(1_035);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: owner,
            node_type: NodeTypeId::new("yssbi.control.branch").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        })
        .unwrap();
    let literal = serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
        value: Value::String("not-a-bool".into()),
    })
    .unwrap();

    let error = plan(
        EditorGraphMutationDto::SetLiteral {
            address: declared(owner, "condition").into(),
            literal: Some(literal),
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert!(error.to_string().contains("literal does not match"));
}

#[test]
fn editor_mutation_rejects_nested_literal_element_mismatch() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let owner = node_id(1_036);
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: owner,
            node_type: NodeTypeId::new("yssbi.data_series.convert.int64_to_string").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        })
        .unwrap();
    let series = TypeExpr::Applied {
        constructor: crate::node_system::protocol::TypeConstructorId::new("core.data_series")
            .unwrap(),
        arguments: vec![TypeExpr::Concrete(TypeId::new("core.int64").unwrap())],
    };
    let literal = serde_json::to_value(crate::node_system::protocol::TypedValue {
        value_type: series,
        value: Value::List(vec![Value::Integer(1), Value::String("wrong".into())]),
    })
    .unwrap();

    let error = plan(
        EditorGraphMutationDto::SetLiteral {
            address: declared(owner, "input").into(),
            literal: Some(literal),
        },
        &document,
        &registry,
    )
    .unwrap_err();

    assert!(error.to_string().contains("literal does not match"));
}

#[test]
fn editor_mutation_rejects_non_user_created_port_templates() {
    let registry = validation_registry();
    let owner = node_id(1_041);
    let document = validation_document(&[owner]);

    for template in ["data_in", "derived_inputs", "not_owned"] {
        assert!(
            plan(
                EditorGraphMutationDto::AddPortInstance {
                    node_id: owner,
                    template: PortKey::new(template).unwrap(),
                    order: None,
                },
                &document,
                &registry,
            )
            .is_err()
        );
    }
    assert!(
        plan(
            EditorGraphMutationDto::RemovePortInstance {
                address: declared(owner, "data_in").into(),
            },
            &document,
            &registry,
        )
        .is_err()
    );
    let derived = derived_address(owner, 1_042);
    let mut with_derived = document.clone();
    with_derived
        .port_bindings
        .insert(derived.clone(), binding());
    assert!(
        plan(
            EditorGraphMutationDto::RemovePortInstance {
                address: derived.into(),
            },
            &with_derived,
            &registry,
        )
        .is_err()
    );
}

#[test]
fn editor_mutation_remove_instance_cleanup_is_reversible() {
    let registry = validation_registry();
    let source = node_id(1_051);
    let owner = node_id(1_052);
    let address = user_address(owner, 1_053);
    let mut document = validation_document(&[source, owner]);
    document
        .bind_port(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("a".into()),
            },
        )
        .unwrap();
    document
        .connect(declared(source, "data_out"), address.clone(), None)
        .unwrap();
    document
        .set_literal(address.clone(), Some(json!(42)))
        .unwrap();
    let before = document.clone();

    let patch = plan(
        EditorGraphMutationDto::RemovePortInstance {
            address: address.clone().into(),
        },
        &document,
        &registry,
    )
    .unwrap();
    document.apply_patch(&patch).unwrap();
    assert!(!document.port_bindings.contains_key(&address));
    assert!(!document.input_states.contains_key(&address));
    assert!(document.connections.is_empty());

    document.apply_patch(&patch.inverse()).unwrap();
    assert_graph_content_eq(&document, &before);
}
