use std::collections::{BTreeMap, BTreeSet};

use crate::package::{
    GraphCompiledPackage, GraphInputBinding, GraphInputSource, GraphObservationIntent,
    GraphOperation, GraphOutputBinding, GraphParameterHandle, GraphParameterPayload,
    GraphParameterScalar, GraphParameterValue, GraphSourceIdentity, GraphValueRef,
};
use crate::{GraphCompileError, GraphCompileErrorCode};
use yss_graph_analysis::{
    GraphPortSemanticFact, GraphSemanticSnapshot, contains_value_dependency_cycle,
    result_category_for_node,
};
use yss_graph_analysis_contract::CompileId;
use yss_graph_document::{
    DocumentConnection, GraphDocument, GraphResourcePath, GraphRevision, NodeId, PortAddress,
};
use yss_graph_protocol::{PortDirection, TypedValue, Value};

const DEBUG_VIEW_NODE_TYPE: &str = "yssbi.debug.view";

struct ResolvedInputContracts {
    ordered_by_node: BTreeMap<NodeId, Box<[PortAddress]>>,
    known_inputs: BTreeSet<PortAddress>,
    values: BTreeMap<PortAddress, TypedValue>,
}

pub struct GraphCompilationInput<'a> {
    document: &'a GraphDocument,
    semantics: &'a GraphSemanticSnapshot,
    expected_revision: GraphRevision,
    graph: GraphResourcePath,
    compile_id: CompileId,
}

impl<'a> GraphCompilationInput<'a> {
    pub fn new(
        document: &'a GraphDocument,
        semantics: &'a GraphSemanticSnapshot,
        expected_revision: GraphRevision,
        graph: GraphResourcePath,
        compile_id: CompileId,
    ) -> Self {
        Self {
            document,
            semantics,
            expected_revision,
            graph,
            compile_id,
        }
    }
}

pub fn compile(
    input: GraphCompilationInput<'_>,
) -> Result<GraphCompiledPackage, GraphCompileError> {
    if input.expected_revision != input.document.revision {
        return Err(GraphCompileError::InvalidGraph {
            graph: input.graph,
            code: GraphCompileErrorCode::InvalidDocument,
        });
    }
    if input.semantics.diagnostics().iter().any(|diagnostic| {
        diagnostic.severity.is_blocking() && diagnostic.code.as_str().starts_with("compiler.type.")
    }) {
        return Err(GraphCompileError::InvalidGraph {
            graph: input.graph,
            code: GraphCompileErrorCode::SemanticTypeUnresolved,
        });
    }
    validate_data_dag(input.document, &input.graph)?;

    lower_package(
        input.document,
        input.semantics,
        input.graph,
        input.compile_id,
    )
}

fn validate_data_dag(
    document: &GraphDocument,
    graph: &GraphResourcePath,
) -> Result<(), GraphCompileError> {
    for connection in document.connections.values() {
        if !document.nodes.contains_key(&connection.input.node_id)
            || !document.nodes.contains_key(&connection.output.node_id)
        {
            return Err(lowering_error(graph));
        }
    }
    if contains_value_dependency_cycle(document) {
        return Err(GraphCompileError::InvalidGraph {
            graph: graph.clone(),
            code: GraphCompileErrorCode::CyclicDataDependency,
        });
    }
    Ok(())
}

fn lower_package(
    document: &GraphDocument,
    semantics: &GraphSemanticSnapshot,
    graph: GraphResourcePath,
    compile_id: CompileId,
) -> Result<GraphCompiledPackage, GraphCompileError> {
    let output_contracts = resolve_output_contracts(semantics, &graph)?;
    let input_contracts = resolve_input_contracts(document, semantics, &graph)?;
    let connections = connections_by_input(document);
    let operations = document
        .nodes
        .values()
        .map(|node| {
            let semantic_node = semantics
                .node(node.id)
                .ok_or_else(|| lowering_error(&graph))?;
            let specialization = semantic_node.specialization.clone().ok_or_else(|| {
                GraphCompileError::InvalidGraph {
                    graph: graph.clone(),
                    code: GraphCompileErrorCode::SemanticTypeUnresolved,
                }
            })?;
            let result_category = result_category_for_node(node.node_type.as_str());
            let parameter_handles = node
                .parameters
                .keys()
                .map(|key| node_parameter_handle(node.id, key.as_str()))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let inputs = lower_input_bindings(
                node.id,
                &input_contracts,
                &connections,
                &output_contracts.value_refs,
                &graph,
            )?;
            let observation_intents: Box<[GraphObservationIntent]> =
                if node.node_type.as_str() == DEBUG_VIEW_NODE_TYPE {
                    inputs
                        .first()
                        .map(|binding| GraphObservationIntent::InspectInput {
                            source: binding.source().clone(),
                        })
                        .into_iter()
                        .collect()
                } else {
                    Box::new([])
                };
            let outputs = output_contracts
                .ordered_by_node
                .get(&node.id)
                .ok_or_else(|| lowering_error(&graph))?
                .iter()
                .map(|address| {
                    output_contracts
                        .value_refs
                        .get(address)
                        .copied()
                        .map(|value| GraphOutputBinding::new(address.to_string(), value))
                        .ok_or_else(|| lowering_error(&graph))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(GraphOperation::new(
                GraphSourceIdentity::new(graph.clone(), Some(node.id), None),
                result_category,
                parameter_handles,
                inputs,
                observation_intents,
                outputs,
                specialization,
            ))
        })
        .collect::<Result<Vec<_>, GraphCompileError>>()?;

    let parameters = lower_parameters(document, &input_contracts.values, &graph)?;
    Ok(GraphCompiledPackage::new(
        graph,
        compile_id,
        operations.into_boxed_slice(),
        parameters,
    ))
}

fn lower_parameters(
    document: &GraphDocument,
    input_values: &BTreeMap<PortAddress, TypedValue>,
    graph_path: &GraphResourcePath,
) -> Result<BTreeMap<GraphParameterHandle, GraphParameterPayload>, GraphCompileError> {
    let mut parameters = BTreeMap::new();
    for node in document.nodes.values() {
        for (key, value) in &node.parameters {
            let handle = node_parameter_handle(node.id, key.as_str());
            let schema = format!("node/{}/{}", node.node_type.as_str(), key.as_str());
            let value = lower_parameter_value(value).map_err(|_| lowering_error(graph_path))?;
            parameters.insert(handle, GraphParameterPayload::new(schema, value));
        }
    }
    for (port, value) in input_values {
        let port = port.to_string();
        let handle = input_parameter_handle(&port);
        let schema = format!("input/{port}");
        let value = lower_protocol_value(&value.value).map_err(|_| lowering_error(graph_path))?;
        parameters.insert(handle, GraphParameterPayload::new(schema, value));
    }
    Ok(parameters)
}

struct ResolvedOutputContracts {
    ordered_by_node: BTreeMap<NodeId, Box<[PortAddress]>>,
    value_refs: BTreeMap<PortAddress, GraphValueRef>,
}

fn resolve_output_contracts(
    semantics: &GraphSemanticSnapshot,
    graph_path: &GraphResourcePath,
) -> Result<ResolvedOutputContracts, GraphCompileError> {
    let mut ordered_by_node = BTreeMap::new();
    let mut ordered_outputs = Vec::new();
    for node in semantics.nodes() {
        let addresses = node
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output && !port.orphan)
            .map(|port| port.address.clone())
            .collect::<Vec<_>>();
        ordered_outputs.extend(addresses.iter().cloned());
        ordered_by_node.insert(node.node_id, addresses.into_boxed_slice());
    }
    let value_refs = ordered_outputs
        .into_iter()
        .enumerate()
        .map(|(index, address)| {
            u32::try_from(index)
                .map(|index| (address, GraphValueRef::new(index)))
                .map_err(|_| lowering_error(graph_path))
        })
        .collect::<Result<_, _>>()?;
    Ok(ResolvedOutputContracts {
        ordered_by_node,
        value_refs,
    })
}

fn lower_input_bindings(
    node_id: NodeId,
    input_contracts: &ResolvedInputContracts,
    connections: &BTreeMap<PortAddress, Vec<&DocumentConnection>>,
    value_refs: &BTreeMap<PortAddress, GraphValueRef>,
    graph_path: &GraphResourcePath,
) -> Result<Box<[GraphInputBinding]>, GraphCompileError> {
    let mut bindings = Vec::new();
    let ports = input_contracts
        .ordered_by_node
        .get(&node_id)
        .ok_or_else(|| lowering_error(graph_path))?;
    for port in ports {
        if !input_contracts.known_inputs.contains(port) {
            return Err(lowering_error(graph_path));
        }
        if let Some(port_connections) = connections.get(port) {
            for connection in port_connections {
                let source = value_refs
                    .get(&connection.output)
                    .copied()
                    .ok_or_else(|| lowering_error(graph_path))?;
                bindings.push(GraphInputBinding::new(
                    port.to_string(),
                    GraphInputSource::Value(source),
                ));
            }
        } else if input_contracts.values.contains_key(port) {
            let port_text = port.to_string();
            bindings.push(GraphInputBinding::new(
                port_text.clone(),
                GraphInputSource::Parameter(input_parameter_handle(&port_text)),
            ));
        }
    }
    Ok(bindings.into_boxed_slice())
}

fn resolve_input_contracts(
    document: &GraphDocument,
    semantics: &GraphSemanticSnapshot,
    graph_path: &GraphResourcePath,
) -> Result<ResolvedInputContracts, GraphCompileError> {
    let connected_inputs = document
        .connections
        .values()
        .map(|connection| connection.input.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut ordered_by_node = BTreeMap::new();
    let mut known_inputs = BTreeSet::new();
    let mut values = BTreeMap::new();

    for node in semantics.nodes() {
        let mut ordered = Vec::new();
        for port in node
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input && !port.orphan)
        {
            resolve_input_port(
                &port.address,
                port,
                document,
                &connected_inputs,
                &mut ordered,
                &mut known_inputs,
                &mut values,
                graph_path,
            )?;
        }
        ordered_by_node.insert(node.node_id, ordered.into_boxed_slice());
    }

    if document
        .connections
        .values()
        .any(|connection| !known_inputs.contains(&connection.input))
        || document
            .input_states
            .keys()
            .any(|address| !known_inputs.contains(address))
    {
        return Err(lowering_error(graph_path));
    }

    Ok(ResolvedInputContracts {
        ordered_by_node,
        known_inputs,
        values,
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_input_port(
    address: &PortAddress,
    port: &GraphPortSemanticFact,
    document: &GraphDocument,
    connected_inputs: &std::collections::BTreeSet<PortAddress>,
    ordered: &mut Vec<PortAddress>,
    known_inputs: &mut BTreeSet<PortAddress>,
    values: &mut BTreeMap<PortAddress, TypedValue>,
    graph_path: &GraphResourcePath,
) -> Result<(), GraphCompileError> {
    if !known_inputs.insert(address.clone()) {
        return Err(lowering_error(graph_path));
    }
    ordered.push(address.clone());
    if connected_inputs.contains(address) {
        return Ok(());
    }
    let value = document
        .input_states
        .get(address)
        .and_then(|state| state.literal_override.clone())
        .or_else(|| port.protocol_default.clone());
    if let Some(value) = value {
        values.insert(address.clone(), value);
    }
    Ok(())
}

fn connections_by_input(
    document: &GraphDocument,
) -> BTreeMap<PortAddress, Vec<&DocumentConnection>> {
    let mut connections = BTreeMap::<_, Vec<_>>::new();
    for connection in document.connections.values() {
        connections
            .entry(connection.input.clone())
            .or_default()
            .push(connection);
    }
    for entries in connections.values_mut() {
        entries.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
    }
    connections
}

fn node_parameter_handle(
    node_id: yss_graph_document::NodeId,
    parameter: &str,
) -> GraphParameterHandle {
    GraphParameterHandle::new(format!("node/{node_id}/{parameter}"))
}

fn input_parameter_handle(port: &str) -> GraphParameterHandle {
    GraphParameterHandle::new(format!("input/{port}"))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ParameterLoweringError {
    NonFiniteDecimal,
    UnsupportedValue,
}

fn lower_protocol_value(value: &Value) -> Result<GraphParameterValue, ParameterLoweringError> {
    Ok(match value {
        Value::Null => GraphParameterValue::Scalar(GraphParameterScalar::Null),
        Value::Bool(value) => GraphParameterValue::Scalar(GraphParameterScalar::Bool(*value)),
        Value::Integer(value) => GraphParameterValue::Scalar(GraphParameterScalar::Integer(*value)),
        Value::Unsigned(value) => {
            GraphParameterValue::Scalar(GraphParameterScalar::Unsigned(*value))
        }
        Value::Decimal(value) => {
            let value = value
                .as_str()
                .parse::<f64>()
                .map_err(|_| ParameterLoweringError::NonFiniteDecimal)?;
            if !value.is_finite() {
                return Err(ParameterLoweringError::NonFiniteDecimal);
            }
            GraphParameterValue::Scalar(GraphParameterScalar::Decimal(value))
        }
        Value::String(value) => {
            GraphParameterValue::Scalar(GraphParameterScalar::String(value.clone()))
        }
        Value::Bytes(values) => GraphParameterValue::List(
            values
                .iter()
                .map(|value| {
                    GraphParameterValue::Scalar(GraphParameterScalar::Unsigned(u64::from(*value)))
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ),
        Value::List(values) => GraphParameterValue::List(
            values
                .iter()
                .map(lower_protocol_value)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        Value::Object(values) => GraphParameterValue::Record(
            values
                .iter()
                .map(|(field, value)| Ok((field.clone(), lower_protocol_value(value)?)))
                .collect::<Result<_, ParameterLoweringError>>()?,
        ),
    })
}

fn lower_parameter_value(
    value: &yss_graph_document::JsonValue,
) -> Result<GraphParameterValue, ParameterLoweringError> {
    if value.is_null() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Null));
    }
    if let Some(value) = value.as_bool() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Bool(
            value,
        )));
    }
    if let Some(value) = value.as_i64() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Integer(
            value,
        )));
    }
    if let Some(value) = value.as_u64() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Unsigned(
            value,
        )));
    }
    if let Some(value) = value.as_f64() {
        if !value.is_finite() {
            return Err(ParameterLoweringError::NonFiniteDecimal);
        }
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Decimal(
            value,
        )));
    }
    if let Some(value) = value.as_str() {
        return Ok(
            if ["events/", "functions/", "variables/", "databases/"]
                .into_iter()
                .any(|prefix| value.starts_with(prefix))
            {
                GraphParameterValue::Resource(value.to_owned().into_boxed_str())
            } else {
                GraphParameterValue::Scalar(GraphParameterScalar::String(
                    value.to_owned().into_boxed_str(),
                ))
            },
        );
    }
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(lower_parameter_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| GraphParameterValue::List(values.into_boxed_slice()));
    }
    if let Some(values) = value.as_object() {
        let fields = values
            .iter()
            .map(|(field, value)| {
                Ok((
                    field.clone().into_boxed_str(),
                    lower_parameter_value(value)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ParameterLoweringError>>()?;
        return Ok(GraphParameterValue::Record(fields));
    }
    Err(ParameterLoweringError::UnsupportedValue)
}

fn lowering_error(graph: &GraphResourcePath) -> GraphCompileError {
    GraphCompileError::InvalidGraph {
        graph: graph.clone(),
        code: GraphCompileErrorCode::LoweringInvariant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_catalog::build_builtin_node_system;
    use yss_graph_document::{
        DocumentConnection, DocumentNode, DynamicPortBinding, InputState, NodePosition, OrderKey,
        ParameterValues, PortInstanceId,
    };
    use yss_graph_protocol::{InputCoercionKind, PortKey, TypeState};
    use yss_graph_registry::NodeRegistryBuilder;
    use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};

    fn graph_path() -> GraphResourcePath {
        GraphResourcePath::new("events/Main.yssbi-event").expect("fixture graph path must be valid")
    }

    fn semantics(
        document: &GraphDocument,
        registry: &yss_graph_registry::NodeRegistry,
    ) -> GraphSemanticSnapshot {
        yss_graph_analysis::resolve_graph_semantics(
            document,
            registry,
            &ResourceCatalogSnapshot::new(
                BTreeMap::new(),
                BTreeMap::new(),
                BTreeMap::new(),
                ResourceCatalogFingerprint::from_bytes([0; 32]),
            ),
        )
    }

    #[test]
    fn empty_document_compiles_to_an_identified_empty_package() {
        let document = GraphDocument::default();
        let registry = NodeRegistryBuilder::new()
            .freeze()
            .expect("an empty test registry is valid");
        let graph = graph_path();
        let compile_id = CompileId::new(7);
        let semantics = semantics(&document, &registry);

        let package = compile(GraphCompilationInput::new(
            &document,
            &semantics,
            document.revision,
            graph.clone(),
            compile_id,
        ))
        .expect("a current empty document must compile");

        assert_eq!(package.graph(), &graph);
        assert_eq!(package.compile_id(), compile_id);
        assert!(package.operations().is_empty());
        assert!(package.parameters().is_empty());
    }

    #[test]
    fn stale_document_revision_is_rejected_before_lowering() {
        let document = GraphDocument::default();
        let registry = NodeRegistryBuilder::new()
            .freeze()
            .expect("an empty test registry is valid");
        let graph = graph_path();
        let stale_revision = GraphRevision::new(document.revision.get().saturating_add(1));
        let semantics = semantics(&document, &registry);

        let error = compile(GraphCompilationInput::new(
            &document,
            &semantics,
            stale_revision,
            graph.clone(),
            CompileId::new(1),
        ))
        .expect_err("a stale document must not compile");

        assert!(matches!(
            error,
            GraphCompileError::InvalidGraph {
                graph: error_graph,
                code: GraphCompileErrorCode::InvalidDocument,
            } if error_graph == graph
        ));
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

        let semantics = semantics(&document, &builtin.registry);
        let package = compile(GraphCompilationInput::new(
            &document,
            &semantics,
            document.revision,
            graph_path(),
            CompileId::new(9),
        ))
        .expect("the dataflow document compiles");
        let producer_operation = package
            .operations()
            .iter()
            .find(|operation| operation.source().node() == Some(producer))
            .expect("producer operation is lowered");
        let fitted_value = producer_operation
            .outputs()
            .iter()
            .find(|output| output.port() == fitted.to_string())
            .expect("fitted output is lowered")
            .value();
        let consumer_operation = package
            .operations()
            .iter()
            .find(|operation| operation.source().node() == Some(consumer))
            .expect("consumer operation is lowered");

        assert!(matches!(
            consumer_operation.inputs()[0].source(),
            GraphInputSource::Value(value) if *value == fitted_value
        ));
        assert_eq!(producer_operation.outputs().len(), 3);
    }

    #[test]
    fn compiler_consumes_the_same_add_type_and_coercion_plan_as_analysis() {
        let builtin = build_builtin_node_system().expect("built-in graph system is valid");
        let integer = NodeId::new();
        let float = NodeId::new();
        let add = NodeId::new();
        let mut document = GraphDocument::default();
        for (node_id, node_type) in [
            (integer, "yssbi.constant.int64"),
            (float, "yssbi.constant.float64"),
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
            .find(|port| {
                port.address == PortAddress::declared(add, PortKey::new("result").unwrap())
            })
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

        let package = compile(GraphCompilationInput::new(
            &document,
            &semantics,
            document.revision,
            graph_path(),
            CompileId::new(11),
        ))
        .expect("the fully solved Add graph compiles");
        let operation = package
            .operations()
            .iter()
            .find(|operation| operation.source().node() == Some(add))
            .expect("Add operation is lowered");

        assert_eq!(operation.kind(), "yssbi.numeric.add");
        assert_eq!(
            operation.specialization().output_types[0].value_type,
            semantic_output
        );
        assert_eq!(
            operation.specialization().coercions.as_ref(),
            [yss_graph_analysis::GraphInputCoercion {
                address: operands[0].clone(),
                kind: InputCoercionKind::WidenInt64ToFloat64,
            }]
        );
    }

    #[test]
    fn normalized_add_literals_resolve_and_lower_as_typed_scalars() {
        let builtin = build_builtin_node_system().expect("built-in graph system is valid");
        let add = NodeId::new();
        let node_type = yss_graph_protocol::NodeTypeId::new("yssbi.numeric.add").unwrap();
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
                        yss_graph_protocol::normalize_json_literal(
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
        let package = compile(GraphCompilationInput::new(
            &document,
            &semantics,
            document.revision,
            graph_path(),
            CompileId::new(12),
        ))
        .expect("the literal Add graph compiles");
        let output_type = semantics
            .node(add)
            .and_then(|node| node.specialization.as_ref())
            .and_then(|specialization| specialization.output_types.first())
            .map(|binding| &binding.value_type);
        let lowered_values = package
            .parameters()
            .values()
            .map(GraphParameterPayload::value)
            .collect::<Vec<_>>();

        assert!(matches!(
            output_type,
            Some(yss_graph_protocol::ResolvedType::Nominal(id)) if id.as_str() == "core.float64"
        ));
        assert!(lowered_values.iter().any(|value| matches!(
            value,
            GraphParameterValue::Scalar(GraphParameterScalar::Integer(1))
        )));
        assert!(lowered_values.iter().any(|value| matches!(
            value,
            GraphParameterValue::Scalar(GraphParameterScalar::Decimal(value)) if *value == 2.5
        )));
    }

    #[test]
    fn compilation_rejects_a_cycle_anywhere_in_the_graph() {
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
        let error = compile(GraphCompilationInput::new(
            &document,
            &semantics,
            document.revision,
            graph_path(),
            CompileId::new(10),
        ))
        .expect_err("the complete Graph must be acyclic");

        assert!(matches!(
            error,
            GraphCompileError::InvalidGraph {
                code: GraphCompileErrorCode::CyclicDataDependency,
                ..
            }
        ));
    }
}
