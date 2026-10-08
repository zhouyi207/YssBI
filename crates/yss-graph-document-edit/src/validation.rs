use crate::DocumentError;
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DynamicPortBinding, GraphDocument, NodeId, PortAddress,
    PortInstanceId,
};
use yss_node_protocol::{PortKey, PortMemberGroupSpec};

pub struct PortMemberGroupState {
    required_templates: BTreeSet<PortKey>,
    present_templates: BTreeMap<PortInstanceId, BTreeSet<PortKey>>,
}

impl PortMemberGroupState {
    pub fn complete_count(&self) -> usize {
        self.present_templates
            .values()
            .filter(|present| present.is_superset(&self.required_templates))
            .count()
    }

    pub fn is_complete(&self, instance_id: PortInstanceId) -> bool {
        self.present_templates
            .get(&instance_id)
            .is_some_and(|present| present.is_superset(&self.required_templates))
    }
}

pub fn port_member_group_state<'a>(
    node_id: NodeId,
    group: &PortMemberGroupSpec,
    bindings: impl IntoIterator<Item = (&'a PortAddress, &'a DynamicPortBinding)>,
) -> PortMemberGroupState {
    let required_templates = group.templates.iter().cloned().collect::<BTreeSet<_>>();
    let mut present_templates = BTreeMap::<PortInstanceId, BTreeSet<PortKey>>::new();
    for (address, binding) in bindings {
        if address.node_id != node_id || !matches!(binding, DynamicPortBinding::UserCreated { .. })
        {
            continue;
        }
        let yss_graph_document::PortRef::Instance {
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

pub fn user_created_port_instance_count<'a>(
    node_id: NodeId,
    template: &PortKey,
    bindings: impl IntoIterator<Item = (&'a PortAddress, &'a DynamicPortBinding)>,
) -> usize {
    bindings
        .into_iter()
        .filter(|(address, binding)| {
            address.node_id == node_id
                && matches!(
                    &address.port,
                    yss_graph_document::PortRef::Instance {
                        template: current,
                        ..
                    } if current == template
                )
                && matches!(binding, DynamicPortBinding::UserCreated { .. })
        })
        .count()
}

pub fn validate_graph_document(document: &GraphDocument) -> Result<(), DocumentError> {
    validate_document_with_connections(document, document.connections.iter())
}

/// Validate removals followed by insertions while borrowing the unchanged document content.
pub fn validate_graph_document_connection_candidate(
    document: &GraphDocument,
    removals: &BTreeMap<ConnectionId, DocumentConnection>,
    insertions: &BTreeMap<ConnectionId, DocumentConnection>,
) -> Result<(), DocumentError> {
    for (id, connection) in removals {
        if id != &connection.id {
            return Err(DocumentError::DuplicateConnection(connection.id));
        }
        crate::patch::validate_connection_removal(document, connection)?;
    }
    for (id, connection) in insertions {
        if id != &connection.id
            || (document.connections.contains_key(id) && !removals.contains_key(id))
        {
            return Err(DocumentError::DuplicateConnection(connection.id));
        }
    }
    let mut retained = document
        .connections
        .iter()
        .filter(|(id, _)| !removals.contains_key(id))
        .peekable();
    let mut inserted = insertions.iter().peekable();
    let connections = std::iter::from_fn(move || match (retained.peek(), inserted.peek()) {
        (Some((left, _)), Some((right, _))) if left < right => retained.next(),
        (Some(_), None) => retained.next(),
        (_, Some(_)) => inserted.next(),
        (None, None) => None,
    });
    validate_document_with_connections(document, connections)
}

fn validate_document_with_connections<'a>(
    document: &GraphDocument,
    connections: impl Iterator<Item = (&'a ConnectionId, &'a DocumentConnection)>,
) -> Result<(), DocumentError> {
    yss_graph_document::validate_constant_definitions(&document.constants)
        .map_err(|error| DocumentError::InvalidConstant(error.id))?;
    for (id, node) in &document.nodes {
        if id != &node.id {
            return Err(DocumentError::DuplicateNode(node.id));
        }
    }
    for address in document.port_bindings.keys() {
        validate_endpoint(document, address)?;
        if !address.is_instance() {
            return Err(DocumentError::UnexpectedPortBinding(address.clone()));
        }
    }
    for address in document.input_states.keys() {
        validate_address(document, address)?;
    }
    for (id, connection) in connections {
        if id != &connection.id {
            return Err(DocumentError::DuplicateConnection(connection.id));
        }
        validate_address(document, &connection.output)?;
        validate_address(document, &connection.input)?;
    }
    Ok(())
}

fn validate_address(document: &GraphDocument, address: &PortAddress) -> Result<(), DocumentError> {
    validate_endpoint(document, address)?;
    if address.is_instance() && !document.port_bindings.contains_key(address) {
        return Err(DocumentError::MissingPortBinding(address.clone()));
    }
    Ok(())
}

fn validate_endpoint(document: &GraphDocument, address: &PortAddress) -> Result<(), DocumentError> {
    if document.nodes.contains_key(&address.node_id) {
        Ok(())
    } else {
        Err(DocumentError::EndpointNodeNotFound(address.node_id))
    }
}
