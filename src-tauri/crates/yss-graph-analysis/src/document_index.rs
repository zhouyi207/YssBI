use std::collections::BTreeMap;
use yss_graph_document::{
    DocumentConnection, DynamicPortBinding, GraphDocument, NodeId, PortAddress,
};

/// Borrowed indexes for one resolution attempt, discarded with that attempt.
/// The document and semantic snapshot remain the only owned graph facts.
pub(crate) struct DocumentIndex<'a> {
    pub incoming: BTreeMap<PortAddress, Vec<&'a DocumentConnection>>,
    bindings: BTreeMap<NodeId, Vec<(&'a PortAddress, &'a DynamicPortBinding)>>,
    connection_counts: BTreeMap<&'a PortAddress, u32>,
}

impl<'a> DocumentIndex<'a> {
    pub fn new(document: &'a GraphDocument) -> Self {
        let mut incoming = BTreeMap::<_, Vec<_>>::new();
        let mut connection_counts = BTreeMap::<_, u32>::new();
        for connection in document.connections.values() {
            incoming
                .entry(connection.input.clone())
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
            bindings,
            connection_counts,
        }
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

    pub fn connection_count(&self, address: &PortAddress) -> u32 {
        self.connection_counts
            .get(address)
            .copied()
            .unwrap_or_default()
    }
}
