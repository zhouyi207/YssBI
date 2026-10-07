use super::*;
use yss_data_contract::{DataValue, SemanticType};
use yss_graph_document::{ConstantId, GraphConstant, normalize_constant_value};
use yss_node_protocol::{
    NodeTypingSpec, ParameterCondition, ParameterEditorSpec, PortDirection, ResolvedType,
    TypeState, TypeUnknownReason,
};

fn constant(id: ConstantId, name: &str, data_type: ValueType, value: DataValue) -> GraphConstant {
    let mut constant = GraphConstant {
        id,
        name: name.into(),
        data_type,
        data_value: value,
        tabular: None,
        description: String::new(),
        tags: vec![],
    };
    normalize_constant_value(&mut constant).unwrap();
    constant
}

fn output(snapshot: &crate::GraphSemanticSnapshot, node: NodeId) -> &crate::GraphPortSemanticFact {
    snapshot
        .node(node)
        .unwrap()
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Output)
        .unwrap()
}

fn assert_column_types(snapshot: &crate::GraphSemanticSnapshot, node: NodeId, added: SemanticType) {
    assert!(
        !snapshot.has_blocking_diagnostics(),
        "{:?}",
        snapshot.diagnostics()
    );
    let fields = &output(snapshot, node).schema_state.exact().unwrap().fields;
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.0.as_ref(), "amount");
    assert_eq!(
        fields[0].scalar_type,
        RelationalScalarType::Known(SemanticType::Numeric)
    );
    assert_eq!(fields[1].name.0.as_ref(), "added");
    assert_eq!(fields[1].scalar_type, RelationalScalarType::Known(added));
}

#[test]
fn constant_defaults_and_visibility_drive_facts_types_and_schema() {
    let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
    let table_id = ConstantId::new();
    let scalar_id = ConstantId::new();
    let override_id = ConstantId::new();
    let protocols = [
        ("tests.constant.table", table_id),
        ("tests.constant.scalar", scalar_id),
    ]
    .map(|(name, id)| {
        let mut protocol = protocol_with_defaults(
            &builtin.registry,
            "yssbi.constant.get",
            name,
            &[("constant", serde_json::json!(id.to_string()))],
        );
        let mut reference = protocol.parameters.groups[0].parameters[0].clone();
        reference.key = "selection".parse().unwrap();
        reference.visible_when = Some(ParameterCondition {
            key: "mode".parse().unwrap(),
            values: [DataValue::String("active".into())].into(),
        });
        let mut mode = reference.clone();
        mode.key = "mode".parse().unwrap();
        mode.editor = ParameterEditorSpec::Text { multiline: false };
        mode.visible_when = None;
        mode.default_value.as_mut().unwrap().value = DataValue::String("active".into());
        protocol.parameters.groups[0].parameters = [reference, mode].into();
        protocol.typing = NodeTypingSpec::ConstantOutput {
            parameter: "selection".parse().unwrap(),
            output: "value".parse().unwrap(),
        };
        protocol
    });
    let registry = crate::tests::registry_with_protocols(protocols);
    let mut document = GraphDocument::default();
    for value in [
        constant(
            table_id,
            "Table",
            ValueType::DataFrame,
            DataValue::String(r#"{"amount":[1]}"#.into()),
        ),
        constant(
            scalar_id,
            "Weight",
            ValueType::number(),
            DataValue::Integer(42),
        ),
        constant(
            override_id,
            "Override",
            ValueType::number(),
            DataValue::Integer(9),
        ),
    ] {
        document.constants.insert(value.id, value);
    }
    let table = node(&mut document, "tests.constant.table", &[]);
    let scalar = node(&mut document, "tests.constant.scalar", &[]);
    let consumer = node(
        &mut document,
        "yssbi.dataframe.set_column",
        &[("name", serde_json::json!("added"))],
    );
    connect(
        &mut document,
        port(table, "value"),
        port(consumer, "source"),
    );
    connect(
        &mut document,
        port(scalar, "value"),
        port(consumer, "series"),
    );
    let resources = catalog(None);
    let mut cache = GraphSemanticCache::default();
    let initial = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_column_types(&initial, consumer, SemanticType::Numeric);
    for (node, id, type_id) in [
        (table, table_id, "tabular.dataframe"),
        (scalar, scalar_id, "core.numeric"),
    ] {
        assert_eq!(
            initial.node(node).unwrap().constant.as_ref().unwrap().id,
            id
        );
        assert_eq!(
            initial.node(node).unwrap().instance_title.as_deref(),
            Some(document.constants[&id].name.as_str())
        );
        assert_eq!(
            output(&initial, node).type_state.exact(),
            Some(&ResolvedType::Nominal(type_id.parse().unwrap()))
        );
        assert!(document.nodes[&node].parameters.is_empty());
    }

    document.constants.insert(
        scalar_id,
        constant(
            scalar_id,
            "Label",
            ValueType::Scalar(SemanticType::Text),
            DataValue::String("next".into()),
        ),
    );
    let changed = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_column_types(&changed, consumer, SemanticType::Text);
    assert_eq!(
        changed.node(scalar).unwrap().instance_title.as_deref(),
        Some("Label")
    );
    document.nodes.get_mut(&scalar).unwrap().parameters.insert(
        "selection".parse().unwrap(),
        serde_json::json!(override_id.to_string()),
    );
    let overridden = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_column_types(&overridden, consumer, SemanticType::Numeric);
    assert_eq!(
        overridden
            .node(scalar)
            .unwrap()
            .constant
            .as_ref()
            .unwrap()
            .id,
        override_id
    );

    // Current semantics must ignore both stored references and defaults when hidden.
    for node in [table, scalar] {
        document
            .nodes
            .get_mut(&node)
            .unwrap()
            .parameters
            .insert("mode".parse().unwrap(), serde_json::json!("inactive"));
    }
    let hidden = assert_matches_full(&document, &registry, &resources, &mut cache);
    for node in [table, scalar] {
        assert!(hidden.node(node).unwrap().constant.is_none());
        assert!(output(&hidden, node).type_state.exact().is_none());
    }
    assert!(output(&hidden, table).schema_state.exact().is_none());
    for node in [table, scalar] {
        document
            .nodes
            .get_mut(&node)
            .unwrap()
            .parameters
            .remove(&"mode".parse().unwrap());
    }
    document
        .nodes
        .get_mut(&scalar)
        .unwrap()
        .parameters
        .insert("selection".parse().unwrap(), serde_json::json!(false));
    let invalid = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(invalid.node(scalar).unwrap().constant.is_none());
    assert!(invalid.has_blocking_diagnostics());
    document
        .nodes
        .get_mut(&scalar)
        .unwrap()
        .parameters
        .remove(&"selection".parse().unwrap());
    let removed = document.constants.remove(&scalar_id).unwrap();
    let missing = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert!(missing.node(scalar).unwrap().constant.is_none());
    assert_eq!(
        output(&missing, scalar).type_state,
        TypeState::Unknown(TypeUnknownReason::MissingResource)
    );
    document.constants.insert(scalar_id, removed);
    let restored = assert_matches_full(&document, &registry, &resources, &mut cache);
    assert_column_types(&restored, consumer, SemanticType::Text);
    assert!(document.nodes[&scalar].parameters.is_empty());
}
