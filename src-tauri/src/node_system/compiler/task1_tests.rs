use super::*;
use crate::node_system::analysis::{DiagnosticLocation, ResourceVersionSet};
use crate::node_system::catalog::builtin_bundle_parts_for_test;
use crate::node_system::document::{DocumentNode, GraphDocument, NodeId, NodePosition};
use crate::node_system::protocol::dataframe::{
    FILTER_PREDICATE_TYPE_ID, PROJECT_COLUMNS_TYPE_ID, validate_filter_predicate_json,
    validate_project_columns_json,
};
use crate::node_system::protocol::{
    NodeTypeId, ParameterConstraint, ParameterKey, ParameterSchema, TypeExpr, TypeId,
};
use crate::node_system::registry::{NodeRegistry, NodeRegistryBuilder, RegisteredNode};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> ResourceVersionSet {
        ResourceVersionSet::new()
    }
}

fn nominal_registry() -> NodeRegistry {
    let (mut provider, _, _) = builtin_bundle_parts_for_test().unwrap();
    let source = provider
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.constant.bool")
        .unwrap();
    let implementation = source
        .implementation()
        .cloned()
        .expect("constant node has a leaf implementation");
    let mut protocol = source.protocol().clone();
    protocol.type_id = NodeTypeId::new("yssbi.test.nominal_parameter").unwrap();
    let mut parameter = protocol.parameters.parameters[0].clone();
    parameter.key = ParameterKey::new("columns").unwrap();
    parameter.value_type = TypeExpr::Concrete(TypeId::new(PROJECT_COLUMNS_TYPE_ID).unwrap());
    parameter.default_value = None;
    parameter.constraints = vec![ParameterConstraint::Required];
    protocol.parameters = ParameterSchema::new(vec![parameter]).unwrap();
    let node = RegisteredNode::leaf(Arc::new(protocol), implementation);
    provider.nodes = provider
        .nodes
        .into_vec()
        .into_iter()
        .chain([node])
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder
        .register_nominal_validator(
            TypeId::new(PROJECT_COLUMNS_TYPE_ID).unwrap(),
            TypeId::new(crate::node_system::protocol::dataframe::PROJECT_COLUMNS_VALIDATOR_ID)
                .unwrap(),
            crate::node_system::protocol::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION,
            validate_project_columns_json,
        )
        .unwrap();
    builder
        .register_nominal_validator(
            TypeId::new(FILTER_PREDICATE_TYPE_ID).unwrap(),
            TypeId::new(crate::node_system::protocol::dataframe::FILTER_PREDICATE_VALIDATOR_ID)
                .unwrap(),
            crate::node_system::protocol::dataframe::DATAFRAME_NOMINAL_CODEC_VERSION,
            validate_filter_predicate_json,
        )
        .unwrap();
    builder.freeze().unwrap()
}

fn document(columns: serde_json::Value) -> GraphDocument {
    let node_id = NodeId::from_uuid(Uuid::from_u128(1));
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.test.nominal_parameter").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::from([(ParameterKey::new("columns").unwrap(), columns)]),
            user_label: None,
        },
    );
    document
}

#[test]
fn compiler_normalization_rejects_malformed_nominal_json_at_parameter() {
    let registry = nominal_registry();
    let document = document(serde_json::json!([]));
    let raw_before = serde_json::to_value(&document).unwrap();

    let result = GraphCompiler::new(&registry, &EmptyResources).compile(&document);

    assert!(result.semantic.is_none());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.parameter.invalid"
            && matches!(
                &diagnostic.primary,
                DiagnosticLocation::Parameter { node_id, key }
                    if *node_id == NodeId::from_uuid(Uuid::from_u128(1))
                        && key.as_str() == "columns"
            )
    }));
    assert_eq!(serde_json::to_value(&document).unwrap(), raw_before);
    assert_eq!(
        document.nodes[&NodeId::from_uuid(Uuid::from_u128(1))].parameters
            [&ParameterKey::new("columns").unwrap()],
        serde_json::json!([]),
    );
}

#[test]
fn compiler_normalization_preserves_valid_raw_nominal_json() {
    let registry = nominal_registry();
    let document = document(serde_json::json!(["b", "a"]));

    let result = GraphCompiler::new(&registry, &EmptyResources).compile(&document);

    assert!(
        !result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.parameter.invalid" })
    );
    assert_eq!(
        result.analysis.nodes[0].normalized_parameters[&ParameterKey::new("columns").unwrap()],
        serde_json::json!(["b", "a"]),
    );
}
