//! Forward type resolution: prepare inputs, reuse or solve outputs, and install node facts.
mod cache;
mod constraints;
mod domains;
mod node_rules;

use crate::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphInputCoercion, GraphKernelSpecialization,
    GraphNodeSemanticFact, GraphPortSemanticFact, GraphPortTypeBinding, graph_problem,
};
pub use cache::GraphSemanticCache;
use cache::{
    CachedNodeResolution, node_input_fingerprint, semantic_fingerprint,
    semantic_fingerprint_without_document,
};
pub(crate) use constraints::automatic_output_constraints;
pub use domains::type_patterns_can_connect;
use domains::{
    bind_input_generics, exact_type_expr, expand_pattern, state_from_candidates, state_from_pattern,
};
use node_rules::apply_node_rule;
use std::collections::BTreeMap;
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{GraphDocument, PortAddress};
use yss_node_protocol::{
    PortDirection, TypeConflict, TypeDomain, TypeExpr, TypeState, TypeUnknownReason,
};
use yss_node_registry::{NodeRegistry, TypeRegistry};

pub(crate) fn resolve_node_types(
    document: &GraphDocument,
    index: &super::document_index::DocumentIndex<'_>,
    registry: &NodeRegistry,
    nodes: &mut [GraphNodeSemanticFact],
    automatic_outputs: &BTreeMap<PortAddress, TypeState>,
    cache: &mut GraphSemanticCache,
) -> Vec<GraphDiagnosticFact> {
    cache.reused_nodes = 0;
    let Some(order) = index.topological_order() else {
        initialize_unresolved_ports(nodes);
        return Vec::new();
    };
    let indices = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id, index))
        .collect::<BTreeMap<_, _>>();
    let connections = &index.incoming;
    let mut resolved = BTreeMap::<PortAddress, TypeState>::new();
    let mut diagnostics = Vec::new();

    for &node_id in order {
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
        let port_snapshot = nodes[index].ports.as_ref();
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

        let (generic_bindings, generic_conflicts) = bind_input_generics(port_snapshot, &states);
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
            port_snapshot,
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
                port_snapshot,
                &mut states,
                registry,
                nodes[index]
                    .constant
                    .as_ref()
                    .map(|constant| &constant.data_type),
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
        .filter(|source| accepted_domain.contains(source))
        .cloned()
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        TypeState::Conflict(TypeConflict::InputNotAccepted)
    } else {
        state_from_candidates(candidates)
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
