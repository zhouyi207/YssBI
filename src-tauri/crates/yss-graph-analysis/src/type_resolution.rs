use std::collections::{BTreeMap, BTreeSet, VecDeque};

use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{GraphDocument, NodeId, PortAddress, PortRef};
use yss_node_protocol::{
    InputCoercionKind, NodeTypingSpec, PortDirection, PortKey, PortSelector, ResolvedType,
    ShapeRule, TypeConflict, TypeDomain, TypeExpr, TypeParameterId, TypeState, TypeUnknownReason,
};
use yss_node_registry::{NodeRegistry, TypeRegistry};

use super::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphInputCoercion, GraphKernelSpecialization,
    GraphNodeSemanticFact, GraphPortSemanticFact, GraphPortTypeBinding, graph_problem,
};

const MAX_DOMAIN_SIZE: usize = 128;

mod constraints;

#[derive(Clone, Default)]
pub struct GraphSemanticCache {
    pub(crate) schemas: super::schema_resolution::SchemaCache,
    nodes: BTreeMap<NodeId, CachedNodeResolution>,
    reused_nodes: usize,
}

impl GraphSemanticCache {
    #[cfg(test)]
    pub(crate) const fn reused_nodes(&self) -> usize {
        self.reused_nodes
    }
}

#[derive(Clone)]
struct CachedNodeResolution {
    input_fingerprint: [u8; 32],
    output_states: BTreeMap<PortAddress, TypeState>,
    coercions: Box<[GraphInputCoercion]>,
}

pub(crate) fn resolve_node_types(
    document: &GraphDocument,
    index: &super::document_index::DocumentIndex<'_>,
    registry: &NodeRegistry,
    nodes: &mut [GraphNodeSemanticFact],
    cache: &mut GraphSemanticCache,
) -> Vec<GraphDiagnosticFact> {
    cache.reused_nodes = 0;
    let Some(order) = topological_order(document) else {
        initialize_unresolved_ports(nodes);
        return Vec::new();
    };
    let automatic_outputs =
        constraints::automatic_output_constraints(document, index, registry, nodes);
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id, index))
        .collect::<BTreeMap<_, _>>();
    let connections = &index.incoming;
    let mut resolved = BTreeMap::<PortAddress, TypeState>::new();
    let mut diagnostics = Vec::new();

    for node_id in order {
        let Some(index) = indices.get(&node_id).copied() else {
            continue;
        };
        let Some(document_node) = document.nodes.get(&node_id) else {
            continue;
        };
        let Some(registered) = registry.get(&document_node.node_type) else {
            initialize_node_ports(&mut nodes[index], registry.types());
            continue;
        };
        let protocol = registered.protocol();
        let port_snapshot = nodes[index].ports.to_vec();
        let mut states = BTreeMap::<PortAddress, TypeState>::new();

        for port in port_snapshot
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
        {
            let (state, mut input_diagnostics) =
                resolve_input_state(port, document, connections, &resolved, registry.types());
            states.insert(port.address.clone(), state);
            diagnostics.append(&mut input_diagnostics);
        }

        let (generic_bindings, generic_conflicts) = bind_input_generics(&port_snapshot, &states);
        for parameter in generic_conflicts {
            diagnostics.push(graph_problem(
                GraphDiagnosticKind::TypeGenericConflict,
                GraphDiagnosticLocation::Node(node_id),
                [("type_parameter", parameter.as_str().into())],
            ));
        }

        for port in port_snapshot
            .iter()
            .filter(|port| port.direction == PortDirection::Output)
        {
            states.insert(
                port.address.clone(),
                automatic_outputs
                    .get(&port.address)
                    .cloned()
                    .unwrap_or_else(|| {
                        state_from_pattern(&port.accepted_type, registry.types(), &generic_bindings)
                    }),
            );
        }

        let mut coercions = Vec::new();
        let input_fingerprint = node_input_fingerprint(
            document_node,
            registry
                .catalog_manifest()
                .node_protocols
                .get(&document_node.node_type),
            &port_snapshot,
            &states,
            nodes[index]
                .constant
                .as_ref()
                .map(|constant| &constant.data_type),
        );
        if let Some(cached) = cache
            .nodes
            .get(&node_id)
            .filter(|cached| cached.input_fingerprint == input_fingerprint)
            .cloned()
        {
            states.extend(cached.output_states);
            coercions.extend(cached.coercions);
            cache.reused_nodes = cache.reused_nodes.saturating_add(1);
        } else {
            apply_node_rule(
                &protocol.typing,
                document_node,
                &port_snapshot,
                &mut states,
                registry,
                document,
                &mut coercions,
            );
            cache.nodes.insert(
                node_id,
                CachedNodeResolution {
                    input_fingerprint,
                    output_states: port_snapshot
                        .iter()
                        .filter(|port| port.direction == PortDirection::Output)
                        .filter_map(|port| {
                            states
                                .get(&port.address)
                                .cloned()
                                .map(|state| (port.address.clone(), state))
                        })
                        .collect(),
                    coercions: coercions.clone().into_boxed_slice(),
                },
            );
        }

        for port in &mut nodes[index].ports {
            port.accepted_domain =
                expand_pattern(&port.accepted_type, registry.types(), &generic_bindings)
                    .and_then(TypeDomain::new);
            port.type_state = if port.orphan {
                TypeState::Unknown(TypeUnknownReason::OrphanedPort)
            } else {
                states
                    .get(&port.address)
                    .cloned()
                    .unwrap_or(TypeState::Unknown(
                        TypeUnknownReason::UnsupportedDeclaration,
                    ))
            };
            resolved.insert(port.address.clone(), port.type_state.clone());
        }

        let unresolved_outputs = nodes[index]
            .ports
            .iter()
            .filter(|port| !port.orphan && port.type_state.exact().is_none())
            .map(|port| port.address.clone())
            .collect::<Vec<_>>();
        for address in unresolved_outputs {
            diagnostics.push(graph_problem(
                GraphDiagnosticKind::TypeResolutionIncomplete,
                GraphDiagnosticLocation::Port(address.clone()),
                [("port", address.to_string().into())],
            ));
        }

        nodes[index].specialization = build_specialization(
            registered
                .implementation()
                .map_or(document_node.node_type.as_str(), |implementation| {
                    implementation.implementation_identity()
                }),
            &nodes[index].ports,
            coercions,
        );
        nodes[index].semantic_fingerprint = semantic_fingerprint(document_node, &nodes[index]);
    }

    cache
        .nodes
        .retain(|node_id, _| document.nodes.contains_key(node_id));
    diagnostics
}

fn initialize_unresolved_ports(nodes: &mut [GraphNodeSemanticFact]) {
    for node in nodes {
        for port in &mut node.ports {
            port.type_state = TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream);
        }
        node.specialization = None;
        node.semantic_fingerprint = semantic_fingerprint_without_document(node);
    }
}

fn initialize_node_ports(node: &mut GraphNodeSemanticFact, types: &TypeRegistry) {
    let bindings = BTreeMap::new();
    for port in &mut node.ports {
        if port.orphan {
            port.accepted_domain = None;
            port.type_state = TypeState::Unknown(TypeUnknownReason::OrphanedPort);
            continue;
        }
        port.accepted_domain =
            expand_pattern(&port.accepted_type, types, &bindings).and_then(TypeDomain::new);
        port.type_state = state_from_pattern(&port.accepted_type, types, &bindings);
    }
    node.specialization = None;
    node.semantic_fingerprint = semantic_fingerprint_without_document(node);
}

pub(crate) fn topological_order(document: &GraphDocument) -> Option<Vec<NodeId>> {
    let mut remaining = document
        .nodes
        .keys()
        .map(|node_id| (*node_id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for connection in document.connections.values() {
        let count = remaining.get_mut(&connection.input.node_id)?;
        *count = count.checked_add(1)?;
        dependents
            .entry(connection.output.node_id)
            .or_default()
            .push(connection.input.node_id);
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(node_id, count)| (*count == 0).then_some(*node_id))
        .collect::<VecDeque<_>>();
    let mut order = Vec::with_capacity(remaining.len());
    while let Some(node_id) = ready.pop_front() {
        order.push(node_id);
        for dependent in dependents.get(&node_id).into_iter().flatten() {
            let count = remaining.get_mut(dependent)?;
            *count = count.checked_sub(1)?;
            if *count == 0 {
                ready.push_back(*dependent);
            }
        }
    }
    (order.len() == document.nodes.len()).then_some(order)
}

fn resolve_input_state(
    port: &GraphPortSemanticFact,
    document: &GraphDocument,
    connections: &BTreeMap<PortAddress, Vec<&yss_graph_document::DocumentConnection>>,
    resolved: &BTreeMap<PortAddress, TypeState>,
    types: &TypeRegistry,
) -> (TypeState, Vec<GraphDiagnosticFact>) {
    if port.orphan {
        return (
            TypeState::Unknown(TypeUnknownReason::OrphanedPort),
            Vec::new(),
        );
    }
    if let Some(port_connections) = connections.get(&port.address) {
        let mut accepted_states = Vec::new();
        let mut diagnostics = Vec::new();
        for connection in port_connections {
            let source = resolved
                .get(&connection.output)
                .cloned()
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream));
            let accepted = restrict_to_pattern(&source, &port.accepted_type, types);
            if matches!(accepted, TypeState::Conflict(_)) {
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::TypeConnectionMismatch,
                    GraphDiagnosticLocation::Connection(connection.id),
                    [
                        ("output", connection.output.to_string().into()),
                        ("input", connection.input.to_string().into()),
                    ],
                ));
            }
            accepted_states.push(accepted);
        }
        return (merge_input_states(accepted_states), diagnostics);
    }

    let literal = document
        .input_states
        .get(&port.address)
        .and_then(|state| state.literal_override.as_ref())
        .or(port.protocol_default.as_ref());
    if let Some(literal) = literal {
        let inferred = exact_type_expr(&literal.value_type)
            .map(TypeState::Exact)
            .unwrap_or(TypeState::Unknown(
                TypeUnknownReason::UnsupportedDeclaration,
            ));
        let accepted = restrict_to_pattern(&inferred, &port.accepted_type, types);
        let diagnostics = matches!(accepted, TypeState::Conflict(_))
            .then(|| {
                graph_problem(
                    GraphDiagnosticKind::TypeInputNotAccepted,
                    GraphDiagnosticLocation::Port(port.address.clone()),
                    [("port", port.address.to_string().into())],
                )
            })
            .into_iter()
            .collect();
        return (accepted, diagnostics);
    }

    (
        state_from_pattern(&port.accepted_type, types, &BTreeMap::new()),
        Vec::new(),
    )
}

fn merge_input_states(states: Vec<TypeState>) -> TypeState {
    if states
        .iter()
        .any(|state| matches!(state, TypeState::Conflict(_)))
    {
        return TypeState::Conflict(TypeConflict::InputNotAccepted);
    }
    if states
        .iter()
        .any(|state| matches!(state, TypeState::Unknown(_)))
    {
        return TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream);
    }
    let candidates = states
        .iter()
        .filter_map(TypeState::domain)
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    state_from_candidates(candidates)
}

fn restrict_to_pattern(source: &TypeState, accepted: &TypeExpr, types: &TypeRegistry) -> TypeState {
    let Some(source_domain) = source.domain() else {
        return source.clone();
    };
    let Some(accepted_domain) = expand_pattern(accepted, types, &BTreeMap::new()) else {
        return source.clone();
    };
    let candidates = source_domain
        .iter()
        .filter(|source| {
            accepted_domain
                .iter()
                .any(|target| is_assignable(source, target))
        })
        .cloned()
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        TypeState::Conflict(TypeConflict::InputNotAccepted)
    } else {
        state_from_candidates(candidates)
    }
}

fn is_assignable(source: &ResolvedType, target: &ResolvedType) -> bool {
    if source == target {
        return true;
    }
    match (source, target) {
        (ResolvedType::Nominal(source), ResolvedType::Nominal(target)) => {
            source.as_str() == "core.numeric" && target.as_str() == "core.numeric"
        }
        (
            ResolvedType::Applied {
                constructor: source_constructor,
                arguments: source_arguments,
            },
            ResolvedType::Applied {
                constructor: target_constructor,
                arguments: target_arguments,
            },
        ) => {
            source_constructor == target_constructor
                && source_arguments.len() == target_arguments.len()
                && source_arguments
                    .iter()
                    .zip(target_arguments)
                    .all(|(source, target)| is_assignable(source, target))
        }
        _ => false,
    }
}

fn bind_input_generics(
    ports: &[GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
) -> (
    BTreeMap<TypeParameterId, TypeDomain>,
    BTreeSet<TypeParameterId>,
) {
    let mut bindings = BTreeMap::<TypeParameterId, TypeDomain>::new();
    let mut conflicts = BTreeSet::new();
    for port in ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input)
    {
        let Some(state) = states.get(&port.address) else {
            continue;
        };
        bind_pattern_generics(&port.accepted_type, state, &mut bindings, &mut conflicts);
    }
    (bindings, conflicts)
}

fn bind_pattern_generics(
    pattern: &TypeExpr,
    state: &TypeState,
    bindings: &mut BTreeMap<TypeParameterId, TypeDomain>,
    conflicts: &mut BTreeSet<TypeParameterId>,
) {
    let Some(domain) = state.domain() else {
        return;
    };
    match pattern {
        TypeExpr::Generic(parameter) => {
            let candidates = domain.iter().cloned().collect::<BTreeSet<_>>();
            let merged = bindings
                .get(parameter)
                .map_or(candidates.clone(), |existing| {
                    existing
                        .types()
                        .iter()
                        .filter(|value| candidates.contains(*value))
                        .cloned()
                        .collect()
                });
            if let Some(domain) = TypeDomain::new(merged) {
                bindings.insert(parameter.clone(), domain);
            } else {
                conflicts.insert(parameter.clone());
            }
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } => {
            for (index, argument) in arguments.iter().enumerate() {
                let nested = domain
                    .iter()
                    .filter_map(|value| match value {
                        ResolvedType::Applied {
                            constructor: actual_constructor,
                            arguments: actual_arguments,
                        } if actual_constructor == constructor => actual_arguments.get(index),
                        ResolvedType::Nominal(_) | ResolvedType::Applied { .. } => None,
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !nested.is_empty() {
                    bind_pattern_generics(
                        argument,
                        &state_from_candidates(nested),
                        bindings,
                        conflicts,
                    );
                }
            }
        }
        TypeExpr::Concrete(_) | TypeExpr::Class(_) | TypeExpr::Union(_) | TypeExpr::Unknown => {}
    }
}

fn conversion_target<'a>(
    node: &'a yss_graph_document::DocumentNode,
    parameter: &yss_node_protocol::ParameterKey,
    registry: &'a NodeRegistry,
) -> Option<&'a str> {
    if let Some(value) = node.parameters.get(parameter) {
        return value.as_str();
    }
    let parameter = registry
        .protocol(&node.node_type)?
        .parameters
        .parameters
        .iter()
        .find(|candidate| &candidate.key == parameter)?;
    match &parameter.default_value.as_ref()?.value {
        yss_node_protocol::Value::String(value) => Some(value.as_ref()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_node_rule(
    rule: &NodeTypingSpec,
    node: &yss_graph_document::DocumentNode,
    ports: &[GraphPortSemanticFact],
    states: &mut BTreeMap<PortAddress, TypeState>,
    registry: &NodeRegistry,
    document: &GraphDocument,
    coercions: &mut Vec<GraphInputCoercion>,
) {
    match rule {
        NodeTypingSpec::Fixed => {}
        NodeTypingSpec::ColumnOutput {
            input,
            column,
            output,
        } => {
            let selected = node
                .parameters
                .get(column)
                .and_then(serde_json::Value::as_str);
            let scalar = selected.and_then(|selected| {
                declared_port(ports, input)?
                    .schema_state
                    .exact()?
                    .fields
                    .iter()
                    .find(|field| field.name.0.as_ref() == selected)
                    .map(|field| field.scalar_type)
            });
            let nominal = match scalar {
                Some(yss_node_protocol::RelationalScalarType::Known(semantic)) => {
                    Some(semantic.type_id())
                }
                _ => None,
            };
            let state = nominal
                .map(|nominal| {
                    TypeState::Exact(ResolvedType::Applied {
                        constructor: "core.data_series".parse().expect("static series type"),
                        arguments: Box::new([ResolvedType::Nominal(
                            nominal.parse().expect("static scalar type"),
                        )]),
                    })
                })
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::Identity { input, output } => {
            let state = declared_port(ports, input)
                .and_then(|port| states.get(&port.address))
                .cloned()
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnconnectedInput));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::NumericFold {
            inputs,
            output,
            shape,
        } => {
            let selected = selected_ports(ports, inputs);
            let state = numeric_fold_state(&selected, states, *shape);
            if let Some(output) = declared_port(ports, output) {
                if let Some(result) = state.exact() {
                    coercions.extend(numeric_coercions(&selected, states, result));
                }
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ShapePreservingNumeric { input, output } => {
            let state = declared_port(ports, input)
                .and_then(|port| states.get(&port.address))
                .map(shape_preserving_numeric_state)
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnconnectedInput));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ShapePreservingConversion {
            input,
            parameter,
            output,
        } => {
            let target = conversion_target(node, parameter, registry);
            if target == Some("auto") {
                // The demand pass has already constrained this output before cache lookup.
                // Input meaning must never select the automatic target.
                return;
            }
            let target = target
                .filter(|value| {
                    yss_node_protocol::SemanticType::ALL
                        .iter()
                        .any(|semantic| semantic.type_id() == *value)
                })
                .and_then(|value| yss_node_protocol::TypeId::new(value).ok());
            let input = declared_port(ports, input).and_then(|port| states.get(&port.address));
            let state = match (target, input) {
                (None, _) => TypeState::Conflict(TypeConflict::UnsupportedParameter),
                (Some(target), Some(state)) => match state.domain() {
                    Some(domain) => {
                        state_from_candidates(domain.iter().filter_map(|source| match source {
                            ResolvedType::Nominal(_) => Some(ResolvedType::Nominal(target.clone())),
                            ResolvedType::Applied {
                                constructor,
                                arguments,
                            } if constructor.as_str() == "core.data_series"
                                && arguments.len() == 1 =>
                            {
                                Some(ResolvedType::Applied {
                                    constructor: constructor.clone(),
                                    arguments: Box::new([ResolvedType::Nominal(target.clone())]),
                                })
                            }
                            _ => None,
                        }))
                    }
                    None => state.clone(),
                },
                (_, None) => TypeState::Unknown(TypeUnknownReason::UnconnectedInput),
            };
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ConstantOutput { parameter, output } => {
            let state = match node.parameters.get(parameter) {
                None => TypeState::Conflict(TypeConflict::MissingParameter),
                Some(value) => value
                    .as_str()
                    .and_then(|identity| document.constants.get(&identity.parse().ok()?))
                    .and_then(|contract| {
                        yss_graph_type_mapping::type_expr_from_data_type(&contract.data_type).ok()
                    })
                    .and_then(|value| exact_type_expr(&value))
                    .map(TypeState::Exact)
                    .unwrap_or(TypeState::Unknown(TypeUnknownReason::MissingResource)),
            };
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
    }
}

fn declared_port<'a>(
    ports: &'a [GraphPortSemanticFact],
    key: &PortKey,
) -> Option<&'a GraphPortSemanticFact> {
    ports.iter().find(
        |port| matches!(&port.address.port, PortRef::Declared { key: actual } if actual == key),
    )
}

fn selected_ports<'a>(
    ports: &'a [GraphPortSemanticFact],
    selectors: &[PortSelector],
) -> Vec<&'a GraphPortSemanticFact> {
    selectors
        .iter()
        .flat_map(|selector| {
            ports
                .iter()
                .filter(move |port| match (selector, &port.address.port) {
                    (PortSelector::Declared(expected), PortRef::Declared { key }) => {
                        key == expected
                    }
                    (PortSelector::AllInstances(expected), PortRef::Instance { template, .. }) => {
                        template == expected
                    }
                    _ => false,
                })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
enum NumericShape {
    Scalar,
    Series,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct NumericType {
    shape: NumericShape,
}

fn numeric_fold_state(
    ports: &[&GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    _shape: ShapeRule,
) -> TypeState {
    if ports.is_empty() {
        return TypeState::Unknown(TypeUnknownReason::UnconnectedInput);
    }
    let mut accumulated = BTreeSet::<NumericType>::new();
    for (index, port) in ports.iter().enumerate() {
        let Some(state) = states.get(&port.address) else {
            return TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream);
        };
        let Some(domain) = state.domain() else {
            return match state {
                TypeState::Conflict(_) => TypeState::Conflict(TypeConflict::IncompatibleInputs),
                TypeState::Unknown(reason) => TypeState::Unknown(*reason),
                TypeState::Exact(_) | TypeState::Constrained(_) => {
                    TypeState::Unknown(TypeUnknownReason::UnresolvedUpstream)
                }
            };
        };
        let candidates = domain
            .iter()
            .filter_map(numeric_type)
            .collect::<BTreeSet<_>>();
        if candidates.is_empty() {
            return TypeState::Conflict(TypeConflict::IncompatibleInputs);
        }
        if index == 0 {
            accumulated = candidates;
            continue;
        }
        accumulated = accumulated
            .iter()
            .flat_map(|left| {
                candidates
                    .iter()
                    .map(move |right| join_numeric(*left, *right))
            })
            .collect();
    }
    state_from_candidates(accumulated.into_iter().map(resolved_numeric_type))
}

fn join_numeric(left: NumericType, right: NumericType) -> NumericType {
    NumericType {
        shape: if left.shape == NumericShape::Series || right.shape == NumericShape::Series {
            NumericShape::Series
        } else {
            NumericShape::Scalar
        },
    }
}

fn shape_preserving_numeric_state(input: &TypeState) -> TypeState {
    let Some(domain) = input.domain() else {
        return input.clone();
    };
    let candidates = domain
        .iter()
        .filter_map(numeric_type)
        .map(|value| resolved_numeric_type(NumericType { shape: value.shape }))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        TypeState::Conflict(TypeConflict::IncompatibleInputs)
    } else {
        state_from_candidates(candidates)
    }
}

fn numeric_coercions(
    ports: &[&GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    result: &ResolvedType,
) -> Vec<GraphInputCoercion> {
    let Some(result) = numeric_type(result) else {
        return Vec::new();
    };
    let mut coercions = Vec::new();
    for port in ports {
        let Some(input) = states.get(&port.address).and_then(TypeState::exact) else {
            continue;
        };
        let Some(input) = numeric_type(input) else {
            continue;
        };
        if input.shape == NumericShape::Scalar && result.shape == NumericShape::Series {
            coercions.push(GraphInputCoercion {
                address: port.address.clone(),
                kind: InputCoercionKind::BroadcastScalarToSeries,
            });
        }
    }
    coercions
}

fn numeric_type(value: &ResolvedType) -> Option<NumericType> {
    match value {
        ResolvedType::Nominal(id) if id.as_str() == "core.numeric" => Some(NumericType {
            shape: NumericShape::Scalar,
        }),
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == yss_node_protocol::DATA_SERIES_CONSTRUCTOR_ID
            && matches!(arguments.as_ref(), [ResolvedType::Nominal(id)] if id.as_str() == "core.numeric") =>
        {
            Some(NumericType {
                shape: NumericShape::Series,
            })
        }
        _ => None,
    }
}

fn resolved_numeric_type(value: NumericType) -> ResolvedType {
    let element = ResolvedType::Nominal(
        yss_node_protocol::TypeId::new("core.numeric").expect("semantic type ID"),
    );
    match value.shape {
        NumericShape::Scalar => element,
        NumericShape::Series => ResolvedType::Applied {
            constructor: yss_node_protocol::TypeConstructorId::new(
                yss_node_protocol::DATA_SERIES_CONSTRUCTOR_ID,
            )
            .expect("series constructor"),
            arguments: Box::new([element]),
        },
    }
}

fn state_from_pattern(
    pattern: &TypeExpr,
    types: &TypeRegistry,
    bindings: &BTreeMap<TypeParameterId, TypeDomain>,
) -> TypeState {
    expand_pattern(pattern, types, bindings).map_or(
        TypeState::Unknown(TypeUnknownReason::UnsupportedDeclaration),
        state_from_candidates,
    )
}

fn expand_pattern(
    pattern: &TypeExpr,
    types: &TypeRegistry,
    bindings: &BTreeMap<TypeParameterId, TypeDomain>,
) -> Option<Vec<ResolvedType>> {
    let values = match pattern {
        TypeExpr::Concrete(id) => vec![ResolvedType::Nominal(id.clone())],
        TypeExpr::Class(class) => types
            .class_members(class)
            .map(|registration| ResolvedType::Nominal(registration.id.clone()))
            .collect(),
        TypeExpr::Generic(parameter) => bindings.get(parameter)?.types().to_vec(),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => {
            let mut products = vec![Vec::new()];
            for argument in arguments {
                let candidates = expand_pattern(argument, types, bindings)?;
                let mut next = Vec::new();
                for product in &products {
                    for candidate in &candidates {
                        if next.len() >= MAX_DOMAIN_SIZE {
                            return None;
                        }
                        let mut product = product.clone();
                        product.push(candidate.clone());
                        next.push(product);
                    }
                }
                products = next;
            }
            products
                .into_iter()
                .map(|arguments| ResolvedType::Applied {
                    constructor: constructor.clone(),
                    arguments: arguments.into_boxed_slice(),
                })
                .collect()
        }
        TypeExpr::Union(members) => {
            let mut values = Vec::new();
            for member in members {
                values.extend(expand_pattern(member, types, bindings)?);
                if values.len() > MAX_DOMAIN_SIZE {
                    return None;
                }
            }
            values
        }
        TypeExpr::Unknown => return None,
    };
    (!values.is_empty()).then_some(values)
}

fn exact_type_expr(value: &TypeExpr) -> Option<ResolvedType> {
    match value {
        TypeExpr::Concrete(id) => Some(ResolvedType::Nominal(id.clone())),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => Some(ResolvedType::Applied {
            constructor: constructor.clone(),
            arguments: arguments
                .iter()
                .map(exact_type_expr)
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
        }),
        TypeExpr::Class(_) | TypeExpr::Generic(_) | TypeExpr::Union(_) | TypeExpr::Unknown => None,
    }
}

fn state_from_candidates(values: impl IntoIterator<Item = ResolvedType>) -> TypeState {
    let Some(domain) = TypeDomain::new(values) else {
        return TypeState::Unknown(TypeUnknownReason::UnsupportedDeclaration);
    };
    match domain.types() {
        [value] => TypeState::Exact(value.clone()),
        _ => TypeState::Constrained(domain),
    }
}

fn build_specialization(
    implementation: &str,
    ports: &[GraphPortSemanticFact],
    coercions: Vec<GraphInputCoercion>,
) -> Option<GraphKernelSpecialization> {
    let input_types = ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input && !port.orphan)
        .filter_map(|port| {
            port.type_state
                .exact()
                .cloned()
                .map(|value_type| GraphPortTypeBinding {
                    address: port.address.clone(),
                    value_type,
                })
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let output_types = ports
        .iter()
        .filter(|port| port.direction == PortDirection::Output && !port.orphan)
        .map(|port| {
            Some(GraphPortTypeBinding {
                address: port.address.clone(),
                value_type: port.type_state.exact()?.clone(),
            })
        })
        .collect::<Option<Vec<_>>>()?
        .into_boxed_slice();
    Some(GraphKernelSpecialization {
        implementation: implementation.into(),
        input_types,
        output_types,
        coercions: coercions.into_boxed_slice(),
    })
}

fn node_input_fingerprint(
    document_node: &yss_graph_document::DocumentNode,
    protocol_fingerprint: Option<&yss_node_registry::ProtocolFingerprint>,
    ports: &[GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    constant_type: Option<&yss_data_contract::ValueType>,
) -> [u8; 32] {
    let ports = ports
        .iter()
        .map(|port| {
            (
                &port.address,
                port.direction,
                &port.accepted_type,
                port.orphan,
                states.get(&port.address),
                &port.schema_state,
            )
        })
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-node-semantic-input.v1",
        &(
            &document_node.node_type,
            &document_node.parameters,
            protocol_fingerprint,
            constant_type,
            ports,
        ),
    )
    .expect("node semantic inputs are canonically serializable")
}

fn semantic_fingerprint(
    document_node: &yss_graph_document::DocumentNode,
    node: &GraphNodeSemanticFact,
) -> [u8; 32] {
    let ports = node
        .ports
        .iter()
        .map(|port| {
            (
                &port.address,
                &port.accepted_type,
                &port.type_state,
                &port.schema_state,
            )
        })
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-node-semantics.v1",
        &(
            &document_node.node_type,
            &document_node.parameters,
            node.constant
                .as_ref()
                .map(|constant| (&constant.data_type, &constant.data_value, &constant.tabular)),
            ports,
        ),
    )
    .expect("node semantic facts are canonically serializable")
}

fn semantic_fingerprint_without_document(node: &GraphNodeSemanticFact) -> [u8; 32] {
    let ports = node
        .ports
        .iter()
        .map(|port| (&port.address, &port.accepted_type, &port.type_state))
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-node-semantics.unavailable.v1",
        &(&node.node_type, ports),
    )
    .expect("node semantic facts are canonically serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_join_is_commutative_associative_and_idempotent() {
        let values = [
            NumericType {
                shape: NumericShape::Scalar,
            },
            NumericType {
                shape: NumericShape::Scalar,
            },
            NumericType {
                shape: NumericShape::Series,
            },
            NumericType {
                shape: NumericShape::Series,
            },
        ];
        for left in values {
            assert_eq!(join_numeric(left, left), left);
            for right in values {
                assert_eq!(join_numeric(left, right), join_numeric(right, left));
                for third in values {
                    assert_eq!(
                        join_numeric(join_numeric(left, right), third,),
                        join_numeric(left, join_numeric(right, third),)
                    );
                }
            }
        }
    }
}
