use std::collections::{BTreeMap, VecDeque};
use yss_graph_document::{
    DocumentConnection, DynamicPortBinding, GraphDocument, NodeId, PortAddress,
};

/// Borrowed indexes for one resolution attempt, discarded with that attempt.
/// The document and semantic snapshot remain the only owned graph facts.
pub(crate) struct DocumentIndex<'a> {
    pub incoming: BTreeMap<PortAddress, Vec<&'a DocumentConnection>>,
    node_inputs: BTreeMap<NodeId, Vec<&'a DocumentConnection>>,
    bindings: BTreeMap<NodeId, Vec<(&'a PortAddress, &'a DynamicPortBinding)>>,
    connection_counts: BTreeMap<&'a PortAddress, u32>,
    topological_order: Option<Vec<NodeId>>,
}

impl<'a> DocumentIndex<'a> {
    pub fn new(document: &'a GraphDocument) -> Self {
        let mut incoming = BTreeMap::<_, Vec<_>>::new();
        let mut node_inputs = BTreeMap::<_, Vec<_>>::new();
        let mut connection_counts = BTreeMap::<_, u32>::new();
        for connection in document.connections.values() {
            incoming
                .entry(connection.input.clone())
                .or_default()
                .push(connection);
            node_inputs
                .entry(connection.input.node_id)
                .or_default()
                .push(connection);
            *connection_counts.entry(&connection.input).or_default() += 1;
            if connection.input != connection.output {
                *connection_counts.entry(&connection.output).or_default() += 1;
            }
        }
        for connections in incoming.values_mut() {
            connections.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        let mut bindings = BTreeMap::<_, Vec<_>>::new();
        for (address, binding) in &document.port_bindings {
            bindings
                .entry(address.node_id)
                .or_default()
                .push((address, binding));
        }
        Self {
            incoming,
            node_inputs,
            bindings,
            connection_counts,
            topological_order: topological_order(document),
        }
    }

    pub fn topological_order(&self) -> Option<&[NodeId]> {
        self.topological_order.as_deref()
    }

    pub fn node_bindings(&self, node: NodeId) -> &[(&'a PortAddress, &'a DynamicPortBinding)] {
        self.bindings
            .get(&node)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn input_connections(&self, address: &PortAddress) -> &[&'a DocumentConnection] {
        self.incoming
            .get(address)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Preserve document connection order for schema input fingerprints.
    pub fn node_input_connections(&self, node: NodeId) -> &[&'a DocumentConnection] {
        self.node_inputs
            .get(&node)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn connection_count(&self, address: &PortAddress) -> u32 {
        self.connection_counts
            .get(address)
            .copied()
            .unwrap_or_default()
    }
}

fn topological_order(document: &GraphDocument) -> Option<Vec<NodeId>> {
    let mut remaining = document
        .nodes
        .keys()
        .map(|node_id| (*node_id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<NodeId, Vec<NodeId>>::new();
    for connection in document.connections.values() {
        // Missing endpoints are diagnosed on the connection. They cannot
        // create a node dependency or invalidate unrelated resolved branches.
        if !remaining.contains_key(&connection.output.node_id) {
            continue;
        }
        let Some(count) = remaining.get_mut(&connection.input.node_id) else {
            continue;
        };
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
