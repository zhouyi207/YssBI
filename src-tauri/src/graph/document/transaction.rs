#[cfg(test)]
use super::{ConnectionId, DocumentNode, DynamicPortBinding, InputState, NodeId, OrderKey};
use super::{DocumentConnection, DocumentError, GraphDocument, PortAddress, TypedValue};
use crate::graph::protocol::{PortKey, PortMemberGroupSpec};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectiveInputBinding {
    Connections(Vec<super::ConnectionId>),
    Literal(TypedValue),
    ProtocolDefault(TypedValue),
    Unbound,
}

pub(crate) struct PortMemberGroupState {
    required_templates: BTreeSet<PortKey>,
    present_templates: BTreeMap<super::PortInstanceId, BTreeSet<PortKey>>,
}

impl PortMemberGroupState {
    pub(crate) fn complete_count(&self) -> usize {
        self.present_templates
            .values()
            .filter(|present| present.is_superset(&self.required_templates))
            .count()
    }

    pub(crate) fn is_complete(&self, instance_id: super::PortInstanceId) -> bool {
        self.present_templates
            .get(&instance_id)
            .is_some_and(|present| present.is_superset(&self.required_templates))
    }

    pub(crate) fn address_is_complete(&self, address: &PortAddress) -> bool {
        match &address.port {
            super::PortRef::Instance { instance_id, .. } => self.is_complete(*instance_id),
            super::PortRef::Declared { .. } => false,
        }
    }
}

pub(crate) fn port_member_group_state<'a>(
    node_id: super::NodeId,
    group: &PortMemberGroupSpec,
    bindings: impl IntoIterator<Item = (&'a PortAddress, &'a super::DynamicPortBinding)>,
) -> PortMemberGroupState {
    let required_templates = group.templates.iter().cloned().collect::<BTreeSet<_>>();
    let mut present_templates = BTreeMap::<super::PortInstanceId, BTreeSet<PortKey>>::new();
    for (address, binding) in bindings {
        if address.node_id != node_id
            || !matches!(binding, super::DynamicPortBinding::UserCreated { .. })
        {
            continue;
        }
        let super::PortRef::Instance {
            template,
            instance_id,
        } = &address.port
        else {
            continue;
        };
        if required_templates.contains(template) {
            present_templates
                .entry(*instance_id)
                .or_default()
                .insert(template.clone());
        }
    }
    PortMemberGroupState {
        required_templates,
        present_templates,
    }
}

#[cfg(test)]
impl GraphDocument {
    pub(crate) fn create_node(&mut self, node: DocumentNode) -> Result<(), DocumentError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DocumentError::DuplicateNode(node.id));
        }
        let next_revision = checked_document_revision(self.revision)?;
        self.nodes.insert(node.id, node);
        self.revision = next_revision;
        Ok(())
    }

    pub(crate) fn delete_node(&mut self, node_id: NodeId) -> Result<DocumentNode, DocumentError> {
        if !self.nodes.contains_key(&node_id) {
            return Err(DocumentError::NodeNotFound(node_id));
        }
        let next_revision = checked_document_revision(self.revision)?;

        let node = self.nodes.remove(&node_id).expect("node existence checked");
        self.connections.retain(|_, connection| {
            connection.output.node_id != node_id && connection.input.node_id != node_id
        });
        self.port_bindings
            .retain(|address, _| address.node_id != node_id);
        self.input_states
            .retain(|address, _| address.node_id != node_id);
        self.revision = next_revision;
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
        let next_revision = checked_document_revision(self.revision)?;
        self.port_bindings.insert(address, binding);
        self.revision = next_revision;
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
        let next_revision = checked_document_revision(self.revision)?;

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
        self.revision = next_revision;
        Ok(id)
    }

    pub(crate) fn disconnect(
        &mut self,
        connection_id: ConnectionId,
    ) -> Result<DocumentConnection, DocumentError> {
        if !self.connections.contains_key(&connection_id) {
            return Err(DocumentError::ConnectionNotFound(connection_id));
        }
        let next_revision = checked_document_revision(self.revision)?;
        let connection = self
            .connections
            .remove(&connection_id)
            .expect("connection existence checked");
        self.revision = next_revision;
        Ok(connection)
    }

    pub(crate) fn set_literal(
        &mut self,
        address: PortAddress,
        literal: Option<TypedValue>,
    ) -> Result<(), DocumentError> {
        self.validate_address(&address)?;
        let next_revision = checked_document_revision(self.revision)?;
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
        self.revision = next_revision;
        Ok(())
    }
}

#[cfg(test)]
fn checked_document_revision(
    revision: super::GraphRevision,
) -> Result<super::GraphRevision, DocumentError> {
    revision
        .checked_next()
        .map_err(|error| DocumentError::RevisionExhausted {
            retained: error.retained,
        })
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
