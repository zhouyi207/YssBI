use crate::{DynamicPortBinding, GraphDocument};

/// Hash persisted semantic intent, excluding presentation and unused derived metadata.
pub fn semantic_document_fingerprint(
    document: &GraphDocument,
) -> Result<[u8; 32], yss_canonical_hash::CanonicalEncodingError> {
    let nodes = document
        .nodes
        .iter()
        .map(|(id, node)| (id, &node.node_type, &node.parameters))
        .collect::<Vec<_>>();
    let mut connections = document.connections.values().collect::<Vec<_>>();
    // Match resolved input order, including persisted tie-breaking identity.
    // Hashing an unordered edge set would alias two different input sequences.
    connections.sort_by(|left, right| {
        left.input
            .cmp(&right.input)
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| left.id.cmp(&right.id))
    });
    let connections = connections
        .into_iter()
        .map(|connection| (&connection.output, &connection.input, &connection.order))
        .collect::<Vec<_>>();
    let bindings = document
        .port_bindings
        .iter()
        .filter(|(address, binding)| {
            matches!(binding, DynamicPortBinding::UserCreated { .. })
                || document.input_states.contains_key(*address)
                || document.connections.values().any(|connection| {
                    &connection.output == *address || &connection.input == *address
                })
        })
        .map(|(address, binding)| match binding {
            DynamicPortBinding::UserCreated { order } => (address, None, order),
            DynamicPortBinding::Resolved { origin, order, .. }
            | DynamicPortBinding::Orphan { origin, order, .. } => (address, Some(origin), order),
        })
        .collect::<Vec<_>>();
    yss_canonical_hash::hash_canonical(
        "yssbi.graph-semantic-document.v1",
        &(
            nodes,
            bindings,
            connections,
            document.input_states.iter().collect::<Vec<_>>(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConnectionId, DocumentConnection, NodeId, OrderKey, PortAddress};

    #[test]
    fn semantic_identity_preserves_effective_connection_sequence() {
        let input = PortAddress::declared(NodeId::new(), "input".parse().unwrap());
        let mut document = GraphDocument::default();
        for byte in [1, 2] {
            let id = ConnectionId::from_bytes([byte; 16]);
            document.connections.insert(
                id,
                DocumentConnection {
                    id,
                    input: input.clone(),
                    output: PortAddress::declared(NodeId::new(), "value".parse().unwrap()),
                    order: Some(OrderKey::new("same")),
                },
            );
        }
        let before = semantic_document_fingerprint(&document).unwrap();
        let mut reordered = document.connections.values().cloned().collect::<Vec<_>>();
        let first = reordered[0].output.clone();
        reordered[0].output = reordered[1].output.clone();
        reordered[1].output = first;
        document.connections = reordered
            .into_iter()
            .map(|connection| (connection.id, connection))
            .collect();
        assert_ne!(semantic_document_fingerprint(&document).unwrap(), before);
    }
}
