pub(super) fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::ValueType,
    data_value: yss_data_contract::DataValue,
) {
    let id = yss_graph_document::ConstantId::from_uuid(node.as_uuid());
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: id.to_string(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    let node = document.nodes.get_mut(&node).unwrap();
    node.node_type = "yssbi.constant.get".parse().unwrap();
    node.parameters = ParameterValues::from([(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    )]);
}

pub(super) fn registry_with_protocols(
    protocols: impl IntoIterator<Item = yss_node_protocol::NodeProtocol>,
) -> yss_node_registry::NodeRegistry {
    use yss_node_registry::{
        LeafImplementation, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    };
    let mut builder = NodeRegistryBuilder::new();
    yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests".parse().unwrap());
    provider.nodes = protocols
        .into_iter()
        .map(|protocol| {
            let implementation = LeafImplementation::new(protocol.type_id.as_str());
            RegisteredNode::leaf(std::sync::Arc::new(protocol), implementation)
        })
        .collect();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

use crate::*;
use std::collections::BTreeMap;
use yss_data_contract::ValueType;
use yss_graph_analysis_contract::DiagnosticSeverity;
use yss_graph_analysis_contract::GraphAnalysisBasis;
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{
    ConnectionId, DynamicPortBinding, GraphDocument, NodeId, OrderKey, PortAddress, PortRef,
};
use yss_graph_document::{DocumentConnection, DocumentNode, NodePosition, ParameterValues};
use yss_graph_resource_contract::ResourceCatalogSnapshot;
use yss_node_catalog::build_builtin_node_system;
use yss_node_protocol::{NodeTypeId, ParameterConstraint};
use yss_node_protocol::{
    ParameterEditorSpec, ParameterKey, PortKey, ResolvedType, ResourceDisplayKind, TypeExpr,
    TypeId, TypeState,
};
use yss_node_registry::RegistryFingerprint;

#[test]
fn connection_compatibility_preserves_union_choices_and_generic_container_shapes() {
    let builtin = build_builtin_node_system().unwrap();
    let numeric = TypeExpr::Concrete(TypeId::new("core.numeric").unwrap());
    let binary = TypeExpr::Concrete(TypeId::new("core.binary").unwrap());
    let generic = TypeExpr::Generic(yss_node_protocol::TypeParameterId::new("t").unwrap());
    let series = yss_node_protocol::data_series_type;
    let numeric_output = TypeExpr::Union(vec![numeric.clone(), series(numeric.clone())]);
    for (source, target, expected) in [
        (numeric_output.clone(), binary, false),
        (numeric_output, numeric.clone(), true),
        (generic.clone(), numeric.clone(), true),
        (series(generic.clone()), numeric.clone(), false),
        (series(generic), series(numeric), true),
    ] {
        assert_eq!(
            type_patterns_can_connect(&source, &target, builtin.registry.types()),
            expected,
            "{source:?} -> {target:?}",
        );
    }

    let mut report_connections = Vec::new();
    for (report, fit, consumer) in [
        ("iv.2sls.summary", "iv.2sls.fit", "iv.2sls.summary"),
        ("iv.liml.summary", "iv.liml.fit", "iv.liml.summary"),
        ("panel.summary", "panel.fit", "panel.predict"),
        ("panel.compare", "panel.fit", "panel.summary"),
        ("var.summary", "var.fit", "timeseries.irf"),
        ("var.lag_order", "var.fit", "var.summary"),
        ("vec.summary", "vec.fit", "vec.summary"),
    ] {
        for (source_kind, output_key, expected) in [(fit, "model", true), (report, "result", false)]
        {
            let source_type = NodeTypeId::new(format!("yssbi.statistics.{source_kind}")).unwrap();
            let consumer_type = NodeTypeId::new(format!("yssbi.statistics.{consumer}")).unwrap();
            let source = NodeId::new();
            let target = NodeId::new();
            let connection = ConnectionId::new();
            let mut document = GraphDocument::default();
            for (node, node_type) in [
                (source, source_type.clone()),
                (target, consumer_type.clone()),
            ] {
                document.nodes.insert(
                    node,
                    DocumentNode {
                        id: node,
                        node_type,
                        position: NodePosition { x: 0.0, y: 0.0 },
                        parameters: ParameterValues::new(),
                        user_label: None,
                    },
                );
            }
            document.connections.insert(
                connection,
                DocumentConnection {
                    id: connection,
                    output: PortAddress::declared(source, output_key.parse().unwrap()),
                    input: PortAddress::declared(target, "model".parse().unwrap()),
                    order: None,
                },
            );
            let output = builtin
                .registry
                .protocol(&source_type)
                .unwrap()
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == output_key)
                .unwrap();
            let input = builtin
                .registry
                .protocol(&consumer_type)
                .unwrap()
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == "model")
                .unwrap();
            let compatible = type_patterns_can_connect(
                &output.value_type,
                &input.value_type,
                builtin.registry.types(),
            );
            let semantics =
                resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
            let mismatch = semantics.diagnostics().iter().any(|diagnostic| {
                diagnostic.blocking
                    && diagnostic.code.as_str()
                        == GraphDiagnosticKind::TypeConnectionMismatch.code()
                    && diagnostic.primary == GraphDiagnosticLocation::Connection(connection)
            });
            if expected {
                assert!(compatible && !mismatch, "{source_kind} -> {consumer}");
            } else {
                report_connections.push((source_kind, compatible, mismatch));
            }
        }
    }
    assert!(
        report_connections
            .iter()
            .all(|(_, compatible, mismatch)| !compatible && *mismatch),
        "{report_connections:?}"
    );
}
fn empty_resources() -> ResourceCatalogSnapshot {
    ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new())
}

fn resolved_scalar(id: &str) -> ResolvedType {
    ResolvedType::Nominal(TypeId::new(id).expect("fixture type ID is valid"))
}

fn resolved_series(element: &str) -> ResolvedType {
    ResolvedType::Applied {
        constructor: yss_node_protocol::TypeConstructorId::new(
            yss_node_protocol::DATA_SERIES_CONSTRUCTOR_ID,
        )
        .expect("fixture constructor ID is valid"),
        arguments: Box::new([resolved_scalar(element)]),
    }
}

fn add_result_type(source_types: &[(&str, &str)], reverse_order: bool) -> TypeState {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let add_id = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        add_id,
        DocumentNode {
            id: add_id,
            node_type: NodeTypeId::new("yssbi.numeric.add")
                .expect("built-in Add node type is valid"),
            position: NodePosition { x: 200.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    for (index, (node_type, output_key)) in source_types.iter().enumerate() {
        let source_id = NodeId::new();
        document.nodes.insert(
            source_id,
            DocumentNode {
                id: source_id,
                node_type: NodeTypeId::new(if node_type.starts_with("core.") {
                    "yssbi.constant.get"
                } else {
                    node_type
                })
                .expect("fixture source node type is valid"),
                position: NodePosition {
                    x: 0.0,
                    y: index as f64 * 100.0,
                },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        if *node_type == "core.numeric" {
            set_constant(
                &mut document,
                source_id,
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                yss_data_contract::DataValue::Integer(0),
            );
        }
        let instance = PortAddress::instance(
            add_id,
            PortKey::new("operands").unwrap(),
            yss_graph_document::PortInstanceId::new(),
        );
        let order = if reverse_order {
            source_types.len() - index
        } else {
            index
        };
        document.port_bindings.insert(
            instance.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{order:05}")),
            },
        );
        let connection_id = ConnectionId::new();
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(source_id, PortKey::new(*output_key).unwrap()),
                input: instance,
                order: None,
            },
        );
    }

    let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
    facts
        .node(add_id)
        .and_then(|node| {
            node.ports.iter().find(|port| {
                matches!(
                    &port.address.port,
                    PortRef::Declared { key } if key.as_str() == "result"
                )
            })
        })
        .map(|port| port.type_state.clone())
        .expect("Add result semantic fact is present")
}

#[test]
fn add_resolver_promotes_shape_and_element_independently_of_operand_order() {
    let scalar_int_float = [("core.numeric", "value"), ("core.numeric", "value")];
    assert_eq!(
        add_result_type(&scalar_int_float, false),
        TypeState::Exact(resolved_scalar("core.numeric"))
    );
    assert_eq!(
        add_result_type(&scalar_int_float, true),
        TypeState::Exact(resolved_scalar("core.numeric"))
    );
    assert_eq!(
        add_result_type(
            &[("core.numeric", "value"), ("core.numeric", "value"),],
            false,
        ),
        TypeState::Exact(resolved_scalar("core.numeric"))
    );
    assert_eq!(
        add_result_type(
            &[
                ("yssbi.dataframe.series.int_range", "series"),
                ("core.numeric", "value"),
            ],
            false,
        ),
        TypeState::Exact(resolved_series("core.numeric"))
    );
    assert_eq!(
        add_result_type(
            &[
                ("yssbi.dataframe.series.int_range", "series"),
                ("yssbi.dataframe.series.int_range", "series"),
            ],
            false,
        ),
        TypeState::Exact(resolved_series("core.numeric"))
    );

    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let add = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        add,
        DocumentNode {
            id: add,
            node_type: NodeTypeId::new("yssbi.numeric.add").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    for index in 0..2 {
        document.port_bindings.insert(
            PortAddress::instance(
                add,
                PortKey::new("operands").unwrap(),
                yss_graph_document::PortInstanceId::new(),
            ),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{index:05}")),
            },
        );
    }
    let snapshot = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
    let result = snapshot
        .node(add)
        .unwrap()
        .ports
        .iter()
        .find(|port| port.address == PortAddress::declared(add, PortKey::new("result").unwrap()))
        .unwrap();
    assert!(matches!(result.type_state, TypeState::Constrained(_)));
    assert!(result.type_state.exact().is_none());
}

#[test]
fn incremental_semantics_equal_full_resolution_and_stop_at_unchanged_output_types() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let source = NodeId::new();
    let view = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: NodeTypeId::new("yssbi.constant.get").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::from([(
                ParameterKey::new("value").unwrap(),
                serde_json::json!(1),
            )]),
            user_label: None,
        },
    );
    document.nodes.insert(
        view,
        DocumentNode {
            id: view,
            node_type: NodeTypeId::new("yssbi.debug.view").unwrap(),
            position: NodePosition { x: 200.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    let connection_id = ConnectionId::new();
    document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(source, PortKey::new("value").unwrap()),
            input: PortAddress::declared(view, PortKey::new("data").unwrap()),
            order: None,
        },
    );
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(1),
    );
    let resources = empty_resources();
    let mut cache = GraphSemanticCache::default();
    resolve_graph_semantics_with_cache(&document, &builtin.registry, &resources, &mut cache);
    assert_eq!(cache.reused_nodes(), 0);

    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(2),
    );
    let incremental =
        resolve_graph_semantics_with_cache(&document, &builtin.registry, &resources, &mut cache);
    let full = resolve_graph_semantics(&document, &builtin.registry, &resources);

    assert_eq!(cache.reused_nodes(), 2);
    assert_eq!(incremental, full);
}

#[test]
fn semantic_cache_invalidates_a_constant_when_its_type_changes() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let variable = NodeId::new();
    let id = yss_graph_document::ConstantId::new();
    let mut document = GraphDocument::default();
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: "Threshold".into(),
            data_type: ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            data_value: yss_data_contract::DataValue::Integer(42),
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    document.nodes.insert(
        variable,
        DocumentNode {
            id: variable,
            node_type: NodeTypeId::new("yssbi.constant.get").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::from([(
                ParameterKey::new("constant").unwrap(),
                serde_json::json!(id.to_string()),
            )]),
            user_label: None,
        },
    );
    let resources = empty_resources();
    let mut cache = GraphSemanticCache::default();
    let integer =
        resolve_graph_semantics_with_cache(&document, &builtin.registry, &resources, &mut cache);
    document.constants.get_mut(&id).unwrap().data_type =
        ValueType::Scalar(yss_data_contract::SemanticType::Identifier);
    document.constants.get_mut(&id).unwrap().data_value = yss_data_contract::DataValue::Integer(42);
    let float =
        resolve_graph_semantics_with_cache(&document, &builtin.registry, &resources, &mut cache);
    let output_type = |snapshot: &GraphSemanticSnapshot| {
        snapshot
            .node(variable)
            .unwrap()
            .ports
            .iter()
            .find(|port| {
                port.address == PortAddress::declared(variable, PortKey::new("value").unwrap())
            })
            .unwrap()
            .type_state
            .clone()
    };

    assert_eq!(
        output_type(&integer),
        TypeState::Exact(resolved_scalar("core.numeric"))
    );
    assert_eq!(
        output_type(&float),
        TypeState::Exact(resolved_scalar("core.identifier"))
    );
    assert_eq!(cache.reused_nodes(), 0);
}

#[test]
fn physical_numeric_change_preserves_semantic_connection() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let left = NodeId::new();
    let right = NodeId::new();
    let add = NodeId::new();
    let consumer = NodeId::new();
    let mut document = GraphDocument::default();
    for (node_id, node_type, x) in [
        (left, "yssbi.dataframe.series.int_range", 0.0),
        (right, "yssbi.constant.get", 0.0),
        (add, "yssbi.numeric.add", 200.0),
        (consumer, "yssbi.value.to_numeric", 400.0),
    ] {
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: NodePosition { x, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }

    set_constant(
        &mut document,
        right,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(0),
    );
    for (index, source) in [left, right].into_iter().enumerate() {
        let operand = PortAddress::instance(
            add,
            PortKey::new("operands").unwrap(),
            yss_graph_document::PortInstanceId::new(),
        );
        document.port_bindings.insert(
            operand.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{index:05}")),
            },
        );
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(
                    source,
                    PortKey::new(if source == left { "series" } else { "value" }).unwrap(),
                ),
                input: operand,
                order: None,
            },
        );
    }
    let downstream_connection = ConnectionId::new();
    document.connections.insert(
        downstream_connection,
        DocumentConnection {
            id: downstream_connection,
            output: PortAddress::declared(add, PortKey::new("result").unwrap()),
            input: PortAddress::declared(consumer, PortKey::new("input").unwrap()),
            order: None,
        },
    );

    let compatible = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
    assert!(!compatible.diagnostics().iter().any(|diagnostic| {
        diagnostic.code.as_str() == GraphDiagnosticKind::TypeConnectionMismatch.code()
            && diagnostic.primary == GraphDiagnosticLocation::Connection(downstream_connection)
    }));

    set_constant(
        &mut document,
        right,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Decimal(
            yss_data_contract::DecimalLiteral::try_from(0.0).expect("finite literal"),
        ),
    );
    let widened = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

    assert!(document.connections.contains_key(&downstream_connection));
    assert!(!widened.diagnostics().iter().any(|diagnostic| {
        diagnostic.code.as_str() == GraphDiagnosticKind::TypeConnectionMismatch.code()
            && diagnostic.primary == GraphDiagnosticLocation::Connection(downstream_connection)
    }));
    assert_eq!(
        widened
            .node(add)
            .unwrap()
            .ports
            .iter()
            .find(|port| port.address
                == PortAddress::declared(add, PortKey::new("result").unwrap()))
            .map(|port| &port.type_state),
        Some(&TypeState::Exact(resolved_series("core.numeric")))
    );
}

#[test]
fn fixed_semantic_conversions_preserve_shape_and_reject_incompatible_consumers() {
    let builtin = build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let source = NodeId::new();
    let convert = NodeId::new();
    for (id, kind) in [
        (source, "yssbi.dataframe.series.int_range"),
        (convert, "yssbi.value.to_numeric"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    let connection = ConnectionId::new();
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: PortAddress::declared(source, "series".parse().unwrap()),
            input: PortAddress::declared(convert, "input".parse().unwrap()),
            order: None,
        },
    );
    let output_type = |document: &GraphDocument| {
        let facts = resolve_graph_semantics(document, &builtin.registry, &empty_resources());
        facts
            .node(convert)
            .unwrap()
            .ports
            .iter()
            .find(|port| port.address == PortAddress::declared(convert, "output".parse().unwrap()))
            .unwrap()
            .type_state
            .clone()
    };
    assert_eq!(
        output_type(&document),
        TypeState::Exact(resolved_series("core.numeric"))
    );
    let dummy = NodeId::new();
    document.nodes.insert(
        dummy,
        DocumentNode {
            id: dummy,
            node_type: "yssbi.dataframe.series.annotate_dummy".parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    let dummy_connection = ConnectionId::new();
    document.connections.insert(
        dummy_connection,
        DocumentConnection {
            id: dummy_connection,
            output: PortAddress::declared(convert, "output".parse().unwrap()),
            input: PortAddress::declared(dummy, "source".parse().unwrap()),
            order: None,
        },
    );
    for (target, accepted) in [
        ("core.numeric", false),
        ("core.categorical", true),
        ("core.ordinal", true),
        ("core.binary", true),
        ("core.text", false),
        ("core.datetime", false),
        ("core.identifier", false),
    ] {
        document.nodes.get_mut(&convert).unwrap().node_type =
            format!("yssbi.value.to_{}", target.strip_prefix("core.").unwrap())
                .parse()
                .unwrap();
        let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
        assert_eq!(
            output_type(&document),
            TypeState::Exact(resolved_series(target))
        );
        assert_eq!(facts.ready().is_some(), accepted, "dummy input {target}");
        if accepted {
            assert_eq!(
                facts
                    .node(dummy)
                    .unwrap()
                    .ports
                    .iter()
                    .find(|port| port.address
                        == PortAddress::declared(dummy, "result".parse().unwrap()))
                    .unwrap()
                    .type_state,
                TypeState::Exact(resolved_series(target)),
            );
        } else {
            assert!(facts.diagnostics().iter().any(|diagnostic| {
                diagnostic.code.as_str() == GraphDiagnosticKind::TypeConnectionMismatch.code()
                    && diagnostic.primary == GraphDiagnosticLocation::Connection(dummy_connection)
            }));
        }
    }
    document.connections.remove(&dummy_connection);
    document.nodes.remove(&dummy);
    document.nodes.get_mut(&convert).unwrap().node_type = "yssbi.value.to_text".parse().unwrap();
    assert_eq!(
        output_type(&document),
        TypeState::Exact(resolved_series("core.text"))
    );
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(1),
    );
    document.connections.get_mut(&connection).unwrap().output =
        PortAddress::declared(source, "value".parse().unwrap());
    assert_eq!(
        output_type(&document),
        TypeState::Exact(resolved_scalar("core.text"))
    );
    document.nodes.get_mut(&convert).unwrap().node_type = "yssbi.value.to_binary".parse().unwrap();
    assert_eq!(
        output_type(&document),
        TypeState::Exact(resolved_scalar("core.binary"))
    );
}

#[test]
fn physical_numeric_promotion_is_not_a_graph_semantic_coercion() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let source = NodeId::new();
    let target = NodeId::new();
    let mut document = GraphDocument::default();
    for (node_id, node_type) in [
        (source, "yssbi.constant.get"),
        (target, "yssbi.dataframe.series.inverse_standardize"),
    ] {
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        source,
        ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Integer(0),
    );
    let mean = PortAddress::declared(target, PortKey::new("mean").unwrap());
    let connection_id = ConnectionId::new();
    document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: PortAddress::declared(source, PortKey::new("value").unwrap()),
            input: mean.clone(),
            order: None,
        },
    );

    let snapshot = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());
    let specialization = snapshot
        .node(target)
        .and_then(|node| node.specialization.as_ref())
        .expect("the exact target node is specialized");

    assert!(specialization.coercions.is_empty());
}

#[test]
fn analysis_accepts_neutral_document_and_basis() {
    let basis = GraphAnalysisBasis {
        kernel_fingerprint: [0; 32],
        registry_fingerprint: RegistryFingerprint::from_bytes([4; 32]),
        resource_observations: BTreeMap::new(),
    };
    let analysis = analyze(
        &basis,
        GraphSemanticSnapshot::new([], [], GraphResolutionOutcome::Complete),
    );
    assert!(analysis.semantic_snapshot().nodes().is_empty());
    assert_eq!(analysis.registry_fingerprint(), &[4; 32]);
}

#[test]
fn editor_projection_reports_an_unbound_required_input() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let node_id = NodeId::new();
    let node_type = NodeTypeId::new("yssbi.debug.view").expect("built-in node type is valid");
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );

    let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

    assert_eq!(facts.outcome(), &GraphResolutionOutcome::Incomplete);
    assert!(facts.diagnostics().iter().any(|diagnostic| {
        diagnostic.code.as_str() == GraphDiagnosticKind::InputUnbound.code()
            && diagnostic.blocking
            && diagnostic.severity == DiagnosticSeverity::Warning
            && matches!(
                &diagnostic.primary,
                GraphDiagnosticLocation::Port(address) if address.node_id == node_id
            )
    }));
}

#[test]
fn editor_projection_reports_required_parameters_from_the_protocol() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let (node_type, parameter_key) = builtin
        .registry
        .iter()
        .find_map(|(node_type, _)| {
            builtin
                .registry
                .protocol(node_type)
                .into_iter()
                .flat_map(|protocol| protocol.parameters.iter())
                .find(|parameter| {
                    parameter.default_value.is_none()
                        && parameter
                            .constraints
                            .contains(&ParameterConstraint::Required)
                })
                .map(|parameter| (node_type.clone(), parameter.key.clone()))
        })
        .expect("built-ins include a required parameter");
    let node_id = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );

    let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

    assert!(facts.diagnostics().iter().any(|diagnostic| {
        diagnostic.code.as_str() == GraphDiagnosticKind::ParameterRequired.code()
            && matches!(
                &diagnostic.primary,
                GraphDiagnosticLocation::Parameter { node_id: owner, key }
                    if *owner == node_id && key == &parameter_key
            )
    }));
}

#[test]
fn editor_projection_reports_missing_resources_at_resource_location() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let (node_type, parameter_key, resource_kind) = builtin
        .registry
        .iter()
        .find_map(|(node_type, _)| {
            builtin
                .registry
                .protocol(node_type)
                .into_iter()
                .flat_map(|protocol| protocol.parameters.iter())
                .find_map(|parameter| match &parameter.editor {
                    ParameterEditorSpec::Resource { kind } => {
                        Some((node_type.clone(), parameter.key.clone(), *kind))
                    }
                    _ => None,
                })
        })
        .expect("built-ins include a resource parameter");
    let identity = match resource_kind {
        ResourceDisplayKind::Function => "functions/missing.yssbi-function",
        ResourceDisplayKind::Database => "databases/missing",
    };
    let node_id = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::from([(
                parameter_key,
                serde_json::Value::String(identity.to_owned()),
            )]),
            user_label: None,
        },
    );

    let facts = resolve_graph_semantics(&document, &builtin.registry, &empty_resources());

    assert!(facts.diagnostics().iter().any(|diagnostic| {
        diagnostic.code.as_str() == GraphDiagnosticKind::ResourceResolutionFailed.code()
            && matches!(
                &diagnostic.primary,
                GraphDiagnosticLocation::Resource(resource) if resource.as_ref() == identity
            )
    }));
}

#[test]
fn editor_projection_reports_a_graph_level_value_cycle() {
    let builtin = build_builtin_node_system().expect("built-in node system is valid");
    let left = NodeId::new();
    let right = NodeId::new();
    let mut document = GraphDocument::default();
    for (node_id, x) in [(left, 0.0), (right, 200.0)] {
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.value.to_text")
                    .expect("built-in node type is valid"),
                position: NodePosition { x, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    for (source, target) in [(left, right), (right, left)] {
        let connection_id = ConnectionId::new();
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(
                    source,
                    PortKey::new("output").expect("built-in port key is valid"),
                ),
                input: PortAddress::declared(
                    target,
                    PortKey::new("input").expect("built-in port key is valid"),
                ),
                order: None,
            },
        );
    }

    let independent = NodeId::new();
    document.nodes.insert(
        independent,
        DocumentNode {
            id: independent,
            node_type: "yssbi.constant.pi".parse().unwrap(),
            position: NodePosition { x: 0.0, y: 300.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    let mut cache = crate::GraphSemanticCache::default();
    let facts = crate::resolve_graph_semantics_with_cache(
        &document,
        &builtin.registry,
        &empty_resources(),
        &mut cache,
    );

    assert!(facts.diagnostics().iter().any(|diagnostic| {
        diagnostic.code.as_str() == GraphDiagnosticKind::DependencyValueCycle.code()
            && diagnostic.primary == GraphDiagnosticLocation::Graph
    }));
    assert!(facts.ready().is_none());
    assert!(facts.nodes_ready(&std::collections::BTreeSet::from([independent])));
    assert!(!facts.nodes_ready(&std::collections::BTreeSet::from([left])));
    assert_eq!(
        facts,
        resolve_graph_semantics(&document, &builtin.registry, &empty_resources())
    );
    let removed = *document.connections.keys().next().unwrap();
    document.connections.remove(&removed);
    let repaired = crate::resolve_graph_semantics_with_cache(
        &document,
        &builtin.registry,
        &empty_resources(),
        &mut cache,
    );
    assert_eq!(
        repaired,
        resolve_graph_semantics(&document, &builtin.registry, &empty_resources())
    );
}
