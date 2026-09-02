use std::collections::BTreeMap;

use crate::package::{
    GraphCompiledPackage, GraphInputBinding, GraphInputKind, GraphInputSource,
    GraphObservationIntent, GraphOperation, GraphParameterHandle, GraphParameterPayload,
    GraphParameterScalar, GraphParameterValue, GraphSourceIdentity, GraphValueRef,
};
use crate::{GraphCompileError, GraphCompileErrorCode};
use yss_graph_analysis::result_category_for_node;
use yss_graph_analysis_contract::CompileId;
use yss_graph_document::{
    DocumentConnection, DynamicPortBinding, GraphDocument, GraphResourcePath, GraphRevision,
    NodeId, OrderKey, PortAddress, PortRef, TypedValue,
};
use yss_graph_protocol::{PortDirection, PortKind, PortSpec, protocol_value_to_json};
use yss_graph_registry::NodeRegistry;

const DEBUG_VIEW_NODE_TYPE: &str = "yssbi.debug.view";

struct ResolvedInputContracts {
    ordered_by_node: BTreeMap<NodeId, Box<[PortAddress]>>,
    kinds: BTreeMap<PortAddress, GraphInputKind>,
    values: BTreeMap<PortAddress, TypedValue>,
}

pub struct GraphCompilationInput<'a> {
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
    expected_revision: GraphRevision,
    graph: GraphResourcePath,
    compile_id: CompileId,
}

impl<'a> GraphCompilationInput<'a> {
    pub fn new(
        document: &'a GraphDocument,
        registry: &'a NodeRegistry,
        expected_revision: GraphRevision,
        graph: GraphResourcePath,
        compile_id: CompileId,
    ) -> Self {
        Self {
            document,
            registry,
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

    lower_package(
        input.document,
        input.registry,
        input.graph,
        input.compile_id,
    )
}

fn lower_package(
    document: &GraphDocument,
    registry: &NodeRegistry,
    graph: GraphResourcePath,
    compile_id: CompileId,
) -> Result<GraphCompiledPackage, GraphCompileError> {
    let value_refs = node_value_refs(document, &graph)?;
    let input_contracts = resolve_input_contracts(document, registry, &graph)?;
    let connections = connections_by_input(document);
    let operations = document
        .nodes
        .values()
        .map(|node| {
            let value_ref = value_refs
                .get(&node.id)
                .copied()
                .ok_or_else(|| lowering_error(&graph))?;
            let result_category = result_category_for_node(node.node_type.as_str());
            let parameter_handles = node
                .parameters
                .keys()
                .map(|key| node_parameter_handle(node.id, key.as_str()))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let inputs =
                lower_input_bindings(node.id, &input_contracts, &connections, &value_refs, &graph)?;
            let observation_intents: Box<[GraphObservationIntent]> = if node.node_type.as_str()
                == DEBUG_VIEW_NODE_TYPE
            {
                vec![GraphObservationIntent::InspectInput { input: value_ref }].into_boxed_slice()
            } else {
                Box::new([])
            };
            Ok(GraphOperation::new(
                GraphSourceIdentity::new(graph.clone(), Some(node.id), None),
                node.node_type.as_str(),
                result_category,
                parameter_handles,
                inputs,
                observation_intents,
                Some(value_ref),
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
        let value = lower_parameter_value(value).map_err(|_| lowering_error(graph_path))?;
        parameters.insert(handle, GraphParameterPayload::new(schema, value));
    }
    Ok(parameters)
}

fn node_value_refs(
    document: &GraphDocument,
    graph_path: &GraphResourcePath,
) -> Result<BTreeMap<yss_graph_document::NodeId, GraphValueRef>, GraphCompileError> {
    document
        .nodes
        .keys()
        .copied()
        .enumerate()
        .map(|(index, node_id)| {
            u32::try_from(index)
                .map(|index| (node_id, GraphValueRef::new(index)))
                .map_err(|_| lowering_error(graph_path))
        })
        .collect()
}

fn lower_input_bindings(
    node_id: NodeId,
    input_contracts: &ResolvedInputContracts,
    connections: &BTreeMap<PortAddress, Vec<&DocumentConnection>>,
    value_refs: &BTreeMap<NodeId, GraphValueRef>,
    graph_path: &GraphResourcePath,
) -> Result<Box<[GraphInputBinding]>, GraphCompileError> {
    let mut bindings = Vec::new();
    let ports = input_contracts
        .ordered_by_node
        .get(&node_id)
        .ok_or_else(|| lowering_error(graph_path))?;
    for port in ports {
        let kind = input_contracts
            .kinds
            .get(port)
            .copied()
            .ok_or_else(|| lowering_error(graph_path))?;
        if let Some(port_connections) = connections.get(port) {
            for connection in port_connections {
                let source = value_refs
                    .get(&connection.output.node_id)
                    .copied()
                    .ok_or_else(|| lowering_error(graph_path))?;
                bindings.push(GraphInputBinding::new(
                    port.to_string(),
                    kind,
                    GraphInputSource::Value(source),
                ));
            }
        } else if input_contracts.values.contains_key(port) {
            let port_text = port.to_string();
            bindings.push(GraphInputBinding::new(
                port_text.clone(),
                kind,
                GraphInputSource::Parameter(input_parameter_handle(&port_text)),
            ));
        }
    }
    Ok(bindings.into_boxed_slice())
}

fn resolve_input_contracts(
    document: &GraphDocument,
    registry: &NodeRegistry,
    graph_path: &GraphResourcePath,
) -> Result<ResolvedInputContracts, GraphCompileError> {
    let connected_inputs = document
        .connections
        .values()
        .map(|connection| connection.input.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut ordered_by_node = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut values = BTreeMap::new();

    for node in document.nodes.values() {
        let protocol = registry
            .protocol(&node.node_type)
            .ok_or_else(|| lowering_error(graph_path))?;
        let mut ordered = Vec::new();
        for spec in protocol
            .interface
            .ports
            .iter()
            .filter(|spec| spec.direction == PortDirection::Input)
        {
            let declared = PortAddress::declared(node.id, spec.key.clone());
            resolve_input_port(
                &declared,
                spec,
                document,
                &connected_inputs,
                &mut ordered,
                &mut kinds,
                &mut values,
                graph_path,
            )?;

            let mut instances = document
                .port_bindings
                .iter()
                .filter(|(address, _)| {
                    address.node_id == node.id
                        && matches!(
                            &address.port,
                            PortRef::Instance { template, .. } if template == &spec.key
                        )
                })
                .collect::<Vec<_>>();
            instances.sort_by(|(left_address, left), (right_address, right)| {
                dynamic_port_order(left)
                    .cmp(dynamic_port_order(right))
                    .then_with(|| left_address.cmp(right_address))
            });
            for (address, _) in instances {
                resolve_input_port(
                    address,
                    spec,
                    document,
                    &connected_inputs,
                    &mut ordered,
                    &mut kinds,
                    &mut values,
                    graph_path,
                )?;
            }
        }
        ordered_by_node.insert(node.id, ordered.into_boxed_slice());
    }

    if document
        .connections
        .values()
        .any(|connection| !kinds.contains_key(&connection.input))
        || document
            .input_states
            .keys()
            .any(|address| !kinds.contains_key(address))
    {
        return Err(lowering_error(graph_path));
    }

    Ok(ResolvedInputContracts {
        ordered_by_node,
        kinds,
        values,
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_input_port(
    address: &PortAddress,
    spec: &PortSpec,
    document: &GraphDocument,
    connected_inputs: &std::collections::BTreeSet<PortAddress>,
    ordered: &mut Vec<PortAddress>,
    kinds: &mut BTreeMap<PortAddress, GraphInputKind>,
    values: &mut BTreeMap<PortAddress, TypedValue>,
    graph_path: &GraphResourcePath,
) -> Result<(), GraphCompileError> {
    let kind = match spec.kind {
        PortKind::Data => GraphInputKind::Data,
        PortKind::Control => GraphInputKind::Control,
        PortKind::Effect => GraphInputKind::Effect,
    };
    if kinds.insert(address.clone(), kind).is_some() {
        return Err(lowering_error(graph_path));
    }
    ordered.push(address.clone());
    if kind != GraphInputKind::Data || connected_inputs.contains(address) {
        return Ok(());
    }
    let value = document
        .input_states
        .get(address)
        .and_then(|state| state.literal_override.clone())
        .or_else(|| {
            spec.input_binding
                .as_ref()
                .and_then(|binding| binding.default_value.as_ref())
                .map(|default| protocol_value_to_json(&default.value))
        });
    if let Some(value) = value {
        values.insert(address.clone(), value);
    }
    Ok(())
}

fn dynamic_port_order(binding: &DynamicPortBinding) -> &OrderKey {
    match binding {
        DynamicPortBinding::UserCreated { order }
        | DynamicPortBinding::Resolved { order, .. }
        | DynamicPortBinding::Orphan { order, .. } => order,
    }
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

fn lower_parameter_value(
    value: &yss_graph_document::TypedValue,
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
    use yss_graph_registry::NodeRegistryBuilder;

    fn graph_path() -> GraphResourcePath {
        GraphResourcePath::new("events/Main.yssbi-event").expect("fixture graph path must be valid")
    }

    #[test]
    fn empty_document_compiles_to_an_identified_empty_package() {
        let document = GraphDocument::default();
        let registry = NodeRegistryBuilder::new()
            .freeze()
            .expect("an empty test registry is valid");
        let graph = graph_path();
        let compile_id = CompileId::new(7);

        let package = compile(GraphCompilationInput::new(
            &document,
            &registry,
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

        let error = compile(GraphCompilationInput::new(
            &document,
            &registry,
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
}
