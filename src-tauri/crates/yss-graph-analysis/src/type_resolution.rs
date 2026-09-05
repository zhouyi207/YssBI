use std::collections::{BTreeMap, BTreeSet, VecDeque};

use yss_graph_compiler_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{GraphDocument, NodeId, PortAddress, PortRef};
use yss_graph_protocol::{
    InputCoercionKind, NodeTypingSpec, NumericPromotionRule, PortDirection, PortKey, PortSelector,
    ResolvedType, ShapeRule, TypeConflict, TypeDomain, TypeExpr, TypeParameterId, TypeState,
    TypeUnknownReason,
};
use yss_graph_registry::{NodeRegistry, TypeRegistry};
use yss_graph_resource_contract::{GraphResourceId, ResourceCatalogSnapshot};

use super::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphInputCoercion, GraphKernelSpecialization,
    GraphNodeSemanticFact, GraphPortSemanticFact, GraphPortTypeBinding, graph_problem,
};

const MAX_DOMAIN_SIZE: usize = 128;

#[derive(Clone, Default)]
pub struct GraphSemanticCache {
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
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    nodes: &mut [GraphNodeSemanticFact],
    cache: &mut GraphSemanticCache,
) -> Vec<GraphDiagnosticFact> {
    cache.reused_nodes = 0;
    let Some(order) = topological_order(document) else {
        initialize_unresolved_ports(nodes);
        return Vec::new();
    };
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id, index))
        .collect::<BTreeMap<_, _>>();
    let connections = connections_by_input(document);
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
                resolve_input_state(port, document, &connections, &resolved, registry.types());
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
                state_from_pattern(&port.accepted_type, registry.types(), &generic_bindings),
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
            matches!(protocol.typing, NodeTypingSpec::VariableOutput { .. })
                .then_some(resources.fingerprint().as_bytes()),
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
            coercions.extend(input_assignment_coercions(
                &port_snapshot,
                &states,
                registry.types(),
                &generic_bindings,
            ));
            apply_node_rule(
                &protocol.typing,
                document_node,
                &port_snapshot,
                &mut states,
                registry,
                resources,
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

fn input_assignment_coercions(
    ports: &[GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    types: &TypeRegistry,
    generic_bindings: &BTreeMap<TypeParameterId, TypeDomain>,
) -> Vec<GraphInputCoercion> {
    ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input && !port.orphan)
        .filter_map(|port| {
            let source = states.get(&port.address)?.exact()?;
            let accepted = expand_pattern(&port.accepted_type, types, generic_bindings)?;
            if accepted.iter().any(|target| target == source) {
                return None;
            }
            let source_numeric = numeric_type(source)?;
            accepted
                .iter()
                .filter_map(numeric_type)
                .any(|target| {
                    source_numeric.shape == target.shape
                        && source_numeric.element == NumericElement::Int64
                        && target.element == NumericElement::Float64
                })
                .then(|| GraphInputCoercion {
                    address: port.address.clone(),
                    kind: InputCoercionKind::WidenInt64ToFloat64,
                })
        })
        .collect()
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

fn connections_by_input(
    document: &GraphDocument,
) -> BTreeMap<PortAddress, Vec<&yss_graph_document::DocumentConnection>> {
    let mut by_input = BTreeMap::<PortAddress, Vec<_>>::new();
    for connection in document.connections.values() {
        by_input
            .entry(connection.input.clone())
            .or_default()
            .push(connection);
    }
    for connections in by_input.values_mut() {
        connections.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
    }
    by_input
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
            source.as_str() == "core.int64" && target.as_str() == "core.float64"
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

#[allow(clippy::too_many_arguments)]
fn apply_node_rule(
    rule: &NodeTypingSpec,
    node: &yss_graph_document::DocumentNode,
    ports: &[GraphPortSemanticFact],
    states: &mut BTreeMap<PortAddress, TypeState>,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
    coercions: &mut Vec<GraphInputCoercion>,
) {
    match rule {
        NodeTypingSpec::Fixed => {}
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
            promotion,
            shape,
        } => {
            let selected = selected_ports(ports, inputs);
            let state = numeric_fold_state(&selected, states, *promotion, *shape);
            if let Some(output) = declared_port(ports, output) {
                if let Some(result) = state.exact() {
                    coercions.extend(numeric_coercions(&selected, states, result));
                }
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ShapePreservingFloat { input, output } => {
            let state = declared_port(ports, input)
                .and_then(|port| states.get(&port.address))
                .map(shape_preserving_float_state)
                .unwrap_or(TypeState::Unknown(TypeUnknownReason::UnconnectedInput));
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::ParameterOutput { parameter, output } => {
            let state = match node.parameters.get(parameter) {
                None => TypeState::Conflict(TypeConflict::MissingParameter),
                Some(value) => value
                    .as_str()
                    .and_then(|value| yss_graph_protocol::TypeId::new(value).ok())
                    .filter(|value| registry.types().get(value).is_some())
                    .map(|value| TypeState::Exact(ResolvedType::Nominal(value)))
                    .unwrap_or(TypeState::Conflict(TypeConflict::UnsupportedParameter)),
            };
            if let Some(output) = declared_port(ports, output) {
                states.insert(output.address.clone(), state);
            }
        }
        NodeTypingSpec::VariableOutput { parameter, output } => {
            let state = match node.parameters.get(parameter) {
                None => TypeState::Conflict(TypeConflict::MissingParameter),
                Some(value) => value
                    .as_str()
                    .and_then(|identity| {
                        resources.variable_contract(&GraphResourceId::new(identity))
                    })
                    .and_then(|contract| {
                        yss_graph_type_mapping::type_expr_from_data_type(contract.data_type()).ok()
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
enum NumericElement {
    Int64,
    Float64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct NumericType {
    shape: NumericShape,
    element: NumericElement,
}

fn numeric_fold_state(
    ports: &[&GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    promotion: NumericPromotionRule,
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
                    .map(move |right| join_numeric(*left, *right, promotion))
            })
            .collect();
    }
    state_from_candidates(accumulated.into_iter().map(resolved_numeric_type))
}

fn join_numeric(
    left: NumericType,
    right: NumericType,
    promotion: NumericPromotionRule,
) -> NumericType {
    NumericType {
        shape: if left.shape == NumericShape::Series || right.shape == NumericShape::Series {
            NumericShape::Series
        } else {
            NumericShape::Scalar
        },
        element: if promotion == NumericPromotionRule::Float64
            || left.element == NumericElement::Float64
            || right.element == NumericElement::Float64
        {
            NumericElement::Float64
        } else {
            NumericElement::Int64
        },
    }
}

fn shape_preserving_float_state(input: &TypeState) -> TypeState {
    let Some(domain) = input.domain() else {
        return input.clone();
    };
    let candidates = domain
        .iter()
        .filter_map(numeric_type)
        .map(|value| {
            resolved_numeric_type(NumericType {
                shape: value.shape,
                element: NumericElement::Float64,
            })
        })
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
        if input.element == NumericElement::Int64 && result.element == NumericElement::Float64 {
            coercions.push(GraphInputCoercion {
                address: port.address.clone(),
                kind: InputCoercionKind::WidenInt64ToFloat64,
            });
        }
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
        ResolvedType::Nominal(id) => numeric_element(id.as_str()).map(|element| NumericType {
            shape: NumericShape::Scalar,
            element,
        }),
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == yss_graph_protocol::DATA_SERIES_CONSTRUCTOR_ID => {
            let [ResolvedType::Nominal(element)] = arguments.as_ref() else {
                return None;
            };
            numeric_element(element.as_str()).map(|element| NumericType {
                shape: NumericShape::Series,
                element,
            })
        }
        ResolvedType::Applied { .. } => None,
    }
}

fn numeric_element(value: &str) -> Option<NumericElement> {
    match value {
        "core.int64" => Some(NumericElement::Int64),
        "core.float64" => Some(NumericElement::Float64),
        _ => None,
    }
}

fn resolved_numeric_type(value: NumericType) -> ResolvedType {
    let element = ResolvedType::Nominal(
        yss_graph_protocol::TypeId::new(match value.element {
            NumericElement::Int64 => "core.int64",
            NumericElement::Float64 => "core.float64",
        })
        .expect("built-in numeric type ID is valid"),
    );
    match value.shape {
        NumericShape::Scalar => element,
        NumericShape::Series => ResolvedType::Applied {
            constructor: yss_graph_protocol::TypeConstructorId::new(
                yss_graph_protocol::DATA_SERIES_CONSTRUCTOR_ID,
            )
            .expect("built-in DataSeries constructor ID is valid"),
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
    protocol_fingerprint: Option<&yss_graph_registry::ProtocolFingerprint>,
    ports: &[GraphPortSemanticFact],
    states: &BTreeMap<PortAddress, TypeState>,
    resource_catalog_fingerprint: Option<&[u8; 32]>,
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
            resource_catalog_fingerprint,
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
        &(&document_node.node_type, &document_node.parameters, ports),
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
                element: NumericElement::Int64,
            },
            NumericType {
                shape: NumericShape::Scalar,
                element: NumericElement::Float64,
            },
            NumericType {
                shape: NumericShape::Series,
                element: NumericElement::Int64,
            },
            NumericType {
                shape: NumericShape::Series,
                element: NumericElement::Float64,
            },
        ];
        for left in values {
            assert_eq!(join_numeric(left, left, NumericPromotionRule::Widen), left);
            for right in values {
                assert_eq!(
                    join_numeric(left, right, NumericPromotionRule::Widen),
                    join_numeric(right, left, NumericPromotionRule::Widen)
                );
                for third in values {
                    assert_eq!(
                        join_numeric(
                            join_numeric(left, right, NumericPromotionRule::Widen),
                            third,
                            NumericPromotionRule::Widen,
                        ),
                        join_numeric(
                            left,
                            join_numeric(right, third, NumericPromotionRule::Widen),
                            NumericPromotionRule::Widen,
                        )
                    );
                }
            }
        }
    }
}
