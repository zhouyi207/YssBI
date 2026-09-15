pub(super) fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::DataType,
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
        yss_graph_document::JsonValue::String(id.to_string()),
    )]);
}

use super::*;
use yss_graph_document::GraphDocument;
use yss_graph_document::{
    DocumentConnection, DocumentNode, DynamicPortBinding, InputState, NodeId, NodePosition,
    OrderKey, ParameterValues, PortInstanceId,
};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use yss_node_catalog::build_builtin_node_system;
use yss_node_protocol::{PortKey, TypeState};
use yss_node_registry::NodeRegistryBuilder;

fn graph_path() -> GraphResourcePath {
    GraphResourcePath::new("events/Main.yssbi-event").expect("fixture graph path must be valid")
}

fn semantics(
    document: &GraphDocument,
    registry: &yss_node_registry::NodeRegistry,
) -> GraphSemanticSnapshot {
    yss_graph_analysis::resolve_graph_semantics(
        document,
        registry,
        &ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        ),
    )
}

#[test]
fn empty_document_prepares_an_identified_empty_plan() {
    let document = GraphDocument::default();
    let registry = NodeRegistryBuilder::new()
        .freeze()
        .expect("an empty test registry is valid");
    let graph = graph_path();
    let plan_id = PlanId::from_existing(7);
    let semantics = semantics(&document, &registry);

    let package = build_template(&graph.clone(), plan_id, &semantics)
        .expect("a current empty document must prepare");

    assert_eq!(package.graph.as_str(), graph.as_str());
    assert_eq!(package.plan_id, plan_id);
    assert!(package.plan.operations().is_empty());
    assert!(package.parameters.is_empty());
}

#[test]
fn connections_consume_the_exact_multi_output_port_value() {
    let builtin = build_builtin_node_system().expect("built-in graph system is valid");
    let producer = NodeId::new();
    let consumer = NodeId::new();
    let fitted = PortAddress::declared(
        producer,
        "fitted".parse().expect("built-in port key is valid"),
    );
    let input = PortAddress::declared(
        consumer,
        "data".parse().expect("built-in port key is valid"),
    );
    let connection_id = yss_graph_document::ConnectionId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        producer,
        DocumentNode {
            id: producer,
            node_type: "yssbi.statistics.ols.fit"
                .parse()
                .expect("built-in node type is valid"),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    document.nodes.insert(
        consumer,
        DocumentNode {
            id: consumer,
            node_type: "yssbi.debug.view"
                .parse()
                .expect("built-in node type is valid"),
            position: NodePosition { x: 300.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: fitted.clone(),
            input,
            order: None,
        },
    );

    let source = NodeId::new();
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: "yssbi.dataframe.series.int_range".parse().unwrap(),
            position: NodePosition { x: -200.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    let protocol = builtin
        .registry
        .protocol(&document.nodes[&producer].node_type)
        .unwrap();
    for spec in protocol.interface.ports.iter().filter(|spec| {
        spec.direction == PortDirection::Input
            && spec
                .input_binding
                .as_ref()
                .is_none_or(|binding| binding.default_value.is_none())
    }) {
        let input = match spec.cardinality {
            yss_node_protocol::PortCardinality::Declared => {
                PortAddress::declared(producer, spec.key.clone())
            }
            yss_node_protocol::PortCardinality::UserCreated { .. } => {
                let address =
                    PortAddress::instance(producer, spec.key.clone(), PortInstanceId::new());
                document.port_bindings.insert(
                    address.clone(),
                    DynamicPortBinding::UserCreated {
                        order: OrderKey::new("00000"),
                    },
                );
                address
            }
            _ => continue,
        };
        let id = yss_graph_document::ConnectionId::new();
        let mut value_node = document.nodes[&source].clone();
        value_node.id = NodeId::new();
        let source_output = PortAddress::declared(value_node.id, PortKey::new("series").unwrap());
        document.nodes.insert(value_node.id, value_node);
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: source_output.clone(),
                input,
                order: None,
            },
        );
    }
    document.nodes.remove(&source);
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let semantics =
        yss_graph_analysis::resolve_graph_semantics(&document, &builtin.registry, &resources);
    assert!(semantics.ready().is_some(), "{:?}", semantics.diagnostics());
    let package = build_template(&graph_path(), PlanId::from_existing(9), &semantics)
        .expect("the dataflow document is prepared");
    let producer_operation = package
        .plan
        .operations()
        .iter()
        .find(|operation| {
            operation
                .source()
                .node()
                .is_some_and(|node| node.as_str() == producer.to_string())
        })
        .expect("producer operation is prepared");
    let fitted_value = producer_operation
        .outputs()
        .iter()
        .find(|output| output.output().port().as_str() == fitted.to_string())
        .expect("fitted output is prepared")
        .value();
    let consumer_operation = package
        .plan
        .operations()
        .iter()
        .find(|operation| {
            operation
                .source()
                .node()
                .is_some_and(|node| node.as_str() == consumer.to_string())
        })
        .expect("consumer operation is prepared");

    assert!(matches!(
        consumer_operation.inputs()[0].source(),
        PlanInputSource::Value(value) if *value == fitted_value
    ));
    assert_eq!(producer_operation.outputs().len(), 3);
}

#[test]
fn execution_uses_the_same_add_type_and_coercion_plan_as_analysis() {
    let builtin = build_builtin_node_system().expect("built-in graph system is valid");
    let integer = NodeId::new();
    let float = NodeId::new();
    let add = NodeId::new();
    let mut document = GraphDocument::default();
    for (node_id, node_type) in [
        (integer, "yssbi.constant.get"),
        (float, "yssbi.constant.get"),
        (add, "yssbi.numeric.add"),
    ] {
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: node_type.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    set_constant(
        &mut document,
        integer,
        yss_data_contract::DataType::Int64,
        yss_data_contract::DataValue::Int64(0),
    );
    set_constant(
        &mut document,
        float,
        yss_data_contract::DataType::Float64,
        yss_data_contract::DataValue::Float64(0.0),
    );
    let mut operands = Vec::new();
    for (index, source) in [integer, float].into_iter().enumerate() {
        let operand = PortAddress::instance(
            add,
            PortKey::new("operands").unwrap(),
            PortInstanceId::new(),
        );
        document.port_bindings.insert(
            operand.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{index:05}")),
            },
        );
        let connection_id = yss_graph_document::ConnectionId::new();
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(source, PortKey::new("value").unwrap()),
                input: operand.clone(),
                order: None,
            },
        );
        operands.push(operand);
    }

    let semantics = semantics(&document, &builtin.registry);
    let semantic_node = semantics.node(add).expect("Add semantics are available");
    let semantic_output = semantic_node
        .ports
        .iter()
        .find(|port| port.address == PortAddress::declared(add, PortKey::new("result").unwrap()))
        .and_then(|port| port.type_state.exact())
        .cloned()
        .expect("Add output is exact");
    assert!(matches!(
        semantic_node
            .ports
            .iter()
            .find(|port| port.address == operands[0])
            .map(|port| &port.type_state),
        Some(TypeState::Exact(_))
    ));

    let package = build_template(&graph_path(), PlanId::from_existing(11), &semantics)
        .expect("the fully solved Add graph is prepared");
    let operation = package
        .plan
        .operations()
        .iter()
        .find(|operation| {
            operation
                .source()
                .node()
                .is_some_and(|node| node.as_str() == add.to_string())
        })
        .expect("Add operation is prepared");

    assert_eq!(operation.kernel_id().as_str(), "yssbi.numeric.add");
    assert_eq!(
        operation.node_type().as_str(),
        semantic_node.node_type.as_str()
    );
    assert_eq!(
        operation
            .inputs()
            .iter()
            .map(|binding| binding.port().as_str().to_owned())
            .collect::<Vec<_>>(),
        operands.iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    for (binding, semantic_binding) in operation.inputs().iter().zip(semantic_node.inputs.iter()) {
        assert_eq!(
            binding
                .contract()
                .group
                .as_ref()
                .map(|id| id.as_str().to_owned()),
            semantic_binding.group.map(|id| id.to_string())
        );
        assert_eq!(
            Some(binding.contract().expected_type.clone()),
            semantics
                .concrete_interface()
                .port(&semantic_binding.address)
                .unwrap()
                .type_state
                .exact()
                .and_then(yss_graph_type_mapping::data_type_from_resolved_type)
        );
    }
    assert_eq!(
        operation.inputs()[0].contract().coercions.as_ref(),
        [PlanInputCoercionKind::WidenInt64ToFloat64]
    );
    assert_eq!(
        operation.specialization().output_types()[0].data_type(),
        &yss_graph_type_mapping::data_type_from_resolved_type(&semantic_output).unwrap()
    );
    assert_eq!(
        operation.specialization().coercions(),
        [PlanInputCoercion::new(
            plan_port(&operands[0]),
            PlanInputCoercionKind::WidenInt64ToFloat64
        )]
    );
}

#[test]
fn normalized_add_literals_resolve_and_prepare_as_typed_scalars() {
    let builtin = build_builtin_node_system().expect("built-in graph system is valid");
    let add = NodeId::new();
    let node_type = yss_node_protocol::NodeTypeId::new("yssbi.numeric.add").unwrap();
    let protocol = builtin.registry.protocol(&node_type).unwrap();
    let operand_pattern = &protocol
        .interface
        .ports
        .iter()
        .find(|port| port.key.as_str() == "operands")
        .unwrap()
        .value_type;
    let mut document = GraphDocument::default();
    document.nodes.insert(
        add,
        DocumentNode {
            id: add,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    for (index, raw) in [
        yss_graph_document::JsonValue::from(1),
        yss_graph_document::JsonValue::from(2.5),
    ]
    .into_iter()
    .enumerate()
    {
        let address = PortAddress::instance(
            add,
            PortKey::new("operands").unwrap(),
            PortInstanceId::new(),
        );
        document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey::new(format!("{index:05}")),
            },
        );
        document.input_states.insert(
            address,
            InputState {
                literal_override: Some(
                    yss_node_protocol::normalize_json_literal(
                        &raw,
                        operand_pattern,
                        builtin.registry.as_ref(),
                    )
                    .expect("numeric literal is normalized to an exact type"),
                ),
            },
        );
    }

    let semantics = semantics(&document, &builtin.registry);
    let package = build_template(&graph_path(), PlanId::from_existing(12), &semantics)
        .expect("the literal Add graph is prepared");
    let output_type = semantics
        .node(add)
        .and_then(|node| node.specialization.as_ref())
        .and_then(|specialization| specialization.output_types.first())
        .map(|binding| &binding.value_type);
    let prepared_values = package
        .parameters
        .values()
        .map(PlanParameterPayload::value)
        .collect::<Vec<_>>();

    assert!(matches!(
        output_type,
        Some(yss_node_protocol::ResolvedType::Nominal(id)) if id.as_str() == "core.float64"
    ));
    assert!(prepared_values.iter().any(|value| matches!(
        value,
        PlanParameterValue::Scalar(PlanParameterScalar::Integer(1))
    )));
    assert!(prepared_values.iter().any(|value| matches!(
        value,
        PlanParameterValue::Scalar(PlanParameterScalar::Decimal(value)) if value.value() == 2.5
    )));
}

#[test]
fn plan_preparation_rejects_a_cycle_anywhere_in_the_graph() {
    let builtin = build_builtin_node_system().expect("built-in graph system is valid");
    let left = NodeId::new();
    let right = NodeId::new();
    let mut document = GraphDocument::default();
    for (node_id, x) in [(left, 0.0), (right, 300.0)] {
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: "yssbi.value.convert"
                    .parse()
                    .expect("built-in node type is valid"),
                position: NodePosition { x, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    for (output_node, input_node) in [(left, right), (right, left)] {
        let id = yss_graph_document::ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(
                    output_node,
                    "output".parse().expect("built-in port key is valid"),
                ),
                input: PortAddress::declared(
                    input_node,
                    "input".parse().expect("built-in port key is valid"),
                ),
                order: None,
            },
        );
    }

    let semantics = semantics(&document, &builtin.registry);
    let error = build_template(&graph_path(), PlanId::from_existing(10), &semantics)
        .expect_err("the complete Graph must be acyclic");

    assert!(matches!(error, GraphPlanError::NotReady));
}

mod cache_test;
