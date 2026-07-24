use crate::node_system::document::{ConnectionId, NodeId};
use std::collections::{BTreeMap, BTreeSet};

/// Returns one stable representative connection for every cyclic value SCC.
pub(crate) fn cyclic_value_dependencies(
    edges: &[(ConnectionId, NodeId, NodeId)],
) -> Vec<ConnectionId> {
    let mut adjacency: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
    for &(_, source, target) in edges {
        adjacency.entry(source).or_default().insert(target);
        adjacency.entry(target).or_default();
    }

    let mut first_pass = VisitState::default();
    for &node in adjacency.keys() {
        visit_forward(node, &adjacency, &mut first_pass);
    }

    let mut reverse: BTreeMap<NodeId, BTreeSet<NodeId>> = adjacency
        .keys()
        .copied()
        .map(|node| (node, BTreeSet::new()))
        .collect();
    for (&source, targets) in &adjacency {
        for &target in targets {
            reverse.entry(target).or_default().insert(source);
        }
    }

    let mut assigned = BTreeSet::new();
    let mut components = Vec::new();
    for &node in first_pass.order.iter().rev() {
        if assigned.contains(&node) {
            continue;
        }
        let mut component = BTreeSet::new();
        collect_component(node, &reverse, &mut assigned, &mut component);
        components.push(component);
    }

    components
        .into_iter()
        .filter_map(|component| {
            let cyclic = component.len() > 1
                || component.iter().any(|node| {
                    adjacency
                        .get(node)
                        .is_some_and(|targets| targets.contains(node))
                });
            cyclic.then(|| {
                edges
                    .iter()
                    .filter(|(_, source, target)| {
                        component.contains(source) && component.contains(target)
                    })
                    .map(|(connection, _, _)| *connection)
                    .min()
                    .expect("cyclic component contains an edge")
            })
        })
        .collect()
}

#[derive(Default)]
struct VisitState {
    visited: BTreeSet<NodeId>,
    order: Vec<NodeId>,
}

fn visit_forward(
    node: NodeId,
    adjacency: &BTreeMap<NodeId, BTreeSet<NodeId>>,
    state: &mut VisitState,
) {
    if !state.visited.insert(node) {
        return;
    }
    if let Some(targets) = adjacency.get(&node) {
        for &target in targets {
            visit_forward(target, adjacency, state);
        }
    }
    state.order.push(node);
}

fn collect_component(
    node: NodeId,
    reverse: &BTreeMap<NodeId, BTreeSet<NodeId>>,
    assigned: &mut BTreeSet<NodeId>,
    component: &mut BTreeSet<NodeId>,
) {
    if !assigned.insert(node) {
        return;
    }
    component.insert(node);
    if let Some(predecessors) = reverse.get(&node) {
        for &predecessor in predecessors {
            collect_component(predecessor, reverse, assigned, component);
        }
    }
}
