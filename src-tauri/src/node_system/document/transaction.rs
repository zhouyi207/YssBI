#[cfg(test)]
use super::{ConnectionId, DocumentNode, DynamicPortBinding, InputState, NodeId, OrderKey};
use super::{
    DocumentConnection, DocumentError, EffectiveInputBinding, GraphDocument, PortAddress,
    TypedValue,
};

#[cfg(test)]
impl GraphDocument {
    pub(crate) fn create_node(&mut self, node: DocumentNode) -> Result<(), DocumentError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DocumentError::DuplicateNode(node.id));
        }
        self.nodes.insert(node.id, node);
        self.revision.advance();
        Ok(())
    }

    pub(crate) fn delete_node(&mut self, node_id: NodeId) -> Result<DocumentNode, DocumentError> {
        if !self.nodes.contains_key(&node_id) {
            return Err(DocumentError::NodeNotFound(node_id));
        }

        let node = self.nodes.remove(&node_id).expect("node existence checked");
        self.connections.retain(|_, connection| {
            connection.output.node_id != node_id && connection.input.node_id != node_id
        });
        self.port_bindings
            .retain(|address, _| address.node_id != node_id);
        self.input_states
            .retain(|address, _| address.node_id != node_id);
        self.revision.advance();
        Ok(node)
    }

    pub(crate) fn bind_port(
        &mut self,
        address: PortAddress,
        binding: DynamicPortBinding,
    ) -> Result<(), DocumentError> {
        self.validate_endpoint(&address)?;
        if !address.is_instance() {
            return Err(DocumentError::UnexpectedPortBinding(address));
        }
        if self.port_bindings.contains_key(&address) {
            return Err(DocumentError::DuplicatePortBinding(address));
        }
        self.port_bindings.insert(address, binding);
        self.revision.advance();
        Ok(())
    }

    pub(crate) fn connect(
        &mut self,
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    ) -> Result<ConnectionId, DocumentError> {
        self.validate_address(&output)?;
        self.validate_address(&input)?;

        let id = ConnectionId::new();
        self.connections.insert(
            id,
            DocumentConnection {
                id,
                output,
                input,
                order,
            },
        );
        self.revision.advance();
        Ok(id)
    }

    pub(crate) fn disconnect(
        &mut self,
        connection_id: ConnectionId,
    ) -> Result<DocumentConnection, DocumentError> {
        let connection = self
            .connections
            .remove(&connection_id)
            .ok_or(DocumentError::ConnectionNotFound(connection_id))?;
        self.revision.advance();
        Ok(connection)
    }

    pub(crate) fn set_literal(
        &mut self,
        address: PortAddress,
        literal: Option<TypedValue>,
    ) -> Result<(), DocumentError> {
        self.validate_address(&address)?;
        match literal {
            Some(value) => {
                self.input_states.insert(
                    address,
                    InputState {
                        literal_override: Some(value),
                    },
                );
            }
            None => {
                self.input_states.remove(&address);
            }
        }
        self.revision.advance();
        Ok(())
    }
}

impl GraphDocument {
    pub fn effective_input_binding(
        &self,
        address: &PortAddress,
        protocol_default: Option<TypedValue>,
    ) -> EffectiveInputBinding {
        let mut connections: Vec<&DocumentConnection> = self
            .connections
            .values()
            .filter(|connection| &connection.input == address)
            .collect();
        connections.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });

        if !connections.is_empty() {
            return EffectiveInputBinding::Connections(
                connections.iter().map(|connection| connection.id).collect(),
            );
        }
        if let Some(literal) = self
            .input_states
            .get(address)
            .and_then(|state| state.literal_override.clone())
        {
            return EffectiveInputBinding::Literal(literal);
        }
        protocol_default
            .map(EffectiveInputBinding::ProtocolDefault)
            .unwrap_or(EffectiveInputBinding::Unbound)
    }

    pub fn validate(&self) -> Result<(), DocumentError> {
        for (id, node) in &self.nodes {
            if id != &node.id {
                return Err(DocumentError::DuplicateNode(node.id));
            }
        }
        for address in self.port_bindings.keys() {
            self.validate_endpoint(address)?;
            if !address.is_instance() {
                return Err(DocumentError::UnexpectedPortBinding(address.clone()));
            }
        }
        for address in self.input_states.keys() {
            self.validate_address(address)?;
        }
        for (id, connection) in &self.connections {
            if id != &connection.id {
                return Err(DocumentError::DuplicateConnection(connection.id));
            }
            self.validate_address(&connection.output)?;
            self.validate_address(&connection.input)?;
        }
        Ok(())
    }

    fn validate_address(&self, address: &PortAddress) -> Result<(), DocumentError> {
        self.validate_endpoint(address)?;
        if address.is_instance() && !self.port_bindings.contains_key(address) {
            return Err(DocumentError::MissingPortBinding(address.clone()));
        }
        Ok(())
    }

    fn validate_endpoint(&self, address: &PortAddress) -> Result<(), DocumentError> {
        if self.nodes.contains_key(&address.node_id) {
            Ok(())
        } else {
            Err(DocumentError::EndpointNodeNotFound(address.node_id))
        }
    }
}
