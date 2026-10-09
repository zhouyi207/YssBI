use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DynamicPortBinding, GraphDocument, GraphDocumentOperation,
    PortAddress,
};

pub(crate) struct PortReferenceIndex {
    connections: BTreeMap<PortAddress, BTreeSet<ConnectionId>>,
    unreferenced_bindings: BTreeSet<PortAddress>,
}

impl PortReferenceIndex {
    pub(crate) fn new(document: &GraphDocument) -> Self {
        let mut index = Self {
            connections: BTreeMap::new(),
            unreferenced_bindings: BTreeSet::new(),
        };
        for (id, connection) in &document.connections {
            index.insert(*id, connection);
        }
        for address in document.port_bindings.keys() {
            index.refresh_binding(document, address);
        }
        index
    }

    pub(crate) fn connections_for(&self, address: &PortAddress) -> Option<&BTreeSet<ConnectionId>> {
        self.connections.get(address)
    }

    pub(crate) fn unreferenced_bindings(&self) -> impl Iterator<Item = &PortAddress> {
        self.unreferenced_bindings.iter()
    }

    /// Observe the document after either applying or restoring a successful operation.
    pub(crate) fn update(&mut self, document: &GraphDocument, operation: &GraphDocumentOperation) {
        match operation {
            GraphDocumentOperation::InsertConnection { connection }
            | GraphDocumentOperation::RemoveConnection { connection } => {
                if let Some(current) = document.connections.get(&connection.id) {
                    self.insert(connection.id, current);
                } else {
                    self.remove(connection.id, connection);
                }
                self.refresh_binding(document, &connection.output);
                self.refresh_binding(document, &connection.input);
            }
            GraphDocumentOperation::InsertPortBinding { address, .. }
            | GraphDocumentOperation::RemovePortBinding { address, .. }
            | GraphDocumentOperation::SetInputState { address, .. } => {
                self.refresh_binding(document, address);
            }
            GraphDocumentOperation::SetConstant { .. }
            | GraphDocumentOperation::InsertNode { .. }
            | GraphDocumentOperation::RemoveNode { .. }
            | GraphDocumentOperation::UpdateNode { .. } => {}
        }
    }

    fn insert(&mut self, id: ConnectionId, connection: &DocumentConnection) {
        for address in [&connection.output, &connection.input] {
            self.connections
                .entry(address.clone())
                .or_default()
                .insert(id);
        }
    }

    fn remove(&mut self, id: ConnectionId, connection: &DocumentConnection) {
        for address in [&connection.output, &connection.input] {
            if let Some(connections) = self.connections.get_mut(address) {
                connections.remove(&id);
                if connections.is_empty() {
                    self.connections.remove(address);
                }
            }
        }
    }

    fn refresh_binding(&mut self, document: &GraphDocument, address: &PortAddress) {
        let unreferenced = document
            .port_bindings
            .get(address)
            .is_some_and(|binding| !matches!(binding, DynamicPortBinding::UserCreated { .. }))
            && !self.connections.contains_key(address)
            && !document.input_states.contains_key(address);
        if unreferenced {
            self.unreferenced_bindings.insert(address.clone());
        } else {
            self.unreferenced_bindings.remove(address);
        }
    }
}
