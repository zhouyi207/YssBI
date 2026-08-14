use super::{CompilerRegistry, CompilerSemanticGraph, RegistryNodeBehavior};
use crate::node_system::analysis::{ControlEdge, EffectDependency, SemanticDependency, ValueEdge};
use crate::node_system::document::{ConnectionId, NodeId, PortAddress};
use crate::node_system::registry::TransparentNodeRole;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn collapse_transparent_nodes<R: CompilerRegistry>(
    registry: &R,
    mut graph: CompilerSemanticGraph,
) -> CompilerSemanticGraph {
    let transparent = graph
        .nodes
        .iter()
        .filter_map(|node| {
            matches!(
                registry
                    .resolve(&node.node_type_id)
                    .map(|node| node.behavior),
                Some(RegistryNodeBehavior::Transparent(
                    TransparentNodeRole::Reroute
                ))
            )
            .then_some(node.node_id)
        })
        .collect::<BTreeSet<_>>();
    if transparent.is_empty() {
        return graph;
    }

    let mut value_inputs = BTreeMap::new();
    let mut control_inputs = BTreeMap::new();
    let mut effect_inputs = BTreeMap::new();
    for dependency in graph.dependencies.iter() {
        match dependency {
            SemanticDependency::Value(edge) if transparent.contains(&edge.target.node_id) => {
                value_inputs.insert(edge.target.node_id, edge.clone());
            }
            SemanticDependency::Control(edge) if transparent.contains(&edge.target_node) => {
                control_inputs.insert(edge.target_node, edge.clone());
            }
            SemanticDependency::Effect(edge) if transparent.contains(&edge.successor) => {
                effect_inputs.insert(edge.successor, edge.clone());
            }
            _ => {}
        }
    }

    graph.dependencies = graph
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            SemanticDependency::Value(edge) if !transparent.contains(&edge.target.node_id) => {
                collapse_value_edge(edge, &transparent, &value_inputs)
                    .map(SemanticDependency::Value)
            }
            SemanticDependency::Control(edge) if !transparent.contains(&edge.target_node) => {
                collapse_control_edge(edge, &transparent, &control_inputs)
                    .map(SemanticDependency::Control)
            }
            SemanticDependency::Effect(edge) if !transparent.contains(&edge.successor) => {
                collapse_effect_edge(edge, &transparent, &effect_inputs)
                    .map(SemanticDependency::Effect)
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    graph.nodes = graph
        .nodes
        .into_vec()
        .into_iter()
        .filter(|node| !transparent.contains(&node.node_id))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    graph
        .resolved_schemas
        .retain(|address, _| !transparent.contains(&address.node_id));
    graph
}

fn collapse_value_edge(
    edge: &ValueEdge<PortAddress, ConnectionId>,
    transparent: &BTreeSet<NodeId>,
    inputs: &BTreeMap<NodeId, ValueEdge<PortAddress, ConnectionId>>,
) -> Option<ValueEdge<PortAddress, ConnectionId>> {
    let mut source = edge.source.clone();
    let mut visited = BTreeSet::new();
    while transparent.contains(&source.node_id) {
        if !visited.insert(source.node_id) {
            return None;
        }
        source = inputs.get(&source.node_id)?.source.clone();
    }
    Some(ValueEdge {
        connection_id: edge.connection_id,
        source,
        target: edge.target.clone(),
    })
}

fn collapse_control_edge(
    edge: &ControlEdge<NodeId, PortAddress, ConnectionId>,
    transparent: &BTreeSet<NodeId>,
    inputs: &BTreeMap<NodeId, ControlEdge<NodeId, PortAddress, ConnectionId>>,
) -> Option<ControlEdge<NodeId, PortAddress, ConnectionId>> {
    let mut source_node = edge.source_node;
    let mut source_port = edge.source_port.clone();
    let mut visited = BTreeSet::new();
    while transparent.contains(&source_node) {
        if !visited.insert(source_node) {
            return None;
        }
        let input = inputs.get(&source_node)?;
        source_node = input.source_node;
        source_port = input.source_port.clone();
    }
    Some(ControlEdge {
        connection_id: edge.connection_id,
        source_node,
        source_port,
        target_node: edge.target_node,
        target_port: edge.target_port.clone(),
    })
}

fn collapse_effect_edge(
    edge: &EffectDependency<NodeId>,
    transparent: &BTreeSet<NodeId>,
    inputs: &BTreeMap<NodeId, EffectDependency<NodeId>>,
) -> Option<EffectDependency<NodeId>> {
    let mut predecessor = edge.predecessor;
    let mut visited = BTreeSet::new();
    while transparent.contains(&predecessor) {
        if !visited.insert(predecessor) {
            return None;
        }
        predecessor = inputs.get(&predecessor)?.predecessor;
    }
    Some(EffectDependency {
        predecessor,
        successor: edge.successor,
        effect_key: edge.effect_key.clone(),
    })
}
