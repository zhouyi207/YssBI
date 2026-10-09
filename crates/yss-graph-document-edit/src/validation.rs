use crate::DocumentError;
use crate::port_references::PortReferenceIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use yss_graph_document::{
    ConnectionId, DocumentConnection, DynamicPortBinding, GraphDocument, GraphDocumentOperation,
    NodeId, PortAddress, PortInstanceId,
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

/// A document borrow whose structural proof, when present, is owned by a patch scope.
#[derive(Clone, Copy)]
pub struct GraphDocumentRead<'a> {
    document: &'a GraphDocument,
    validated: bool,
    references: Option<&'a OnceLock<PortReferenceIndex>>,
}

impl<'a> GraphDocumentRead<'a> {
    pub fn new(document: &'a GraphDocument) -> Self {
        Self::from_validity(document, false, None)
    }

    pub fn document(self) -> &'a GraphDocument {
        self.document
    }

    pub fn connections_for<'query>(
        self,
        address: &'query PortAddress,
    ) -> impl Iterator<Item = &'a DocumentConnection> + 'query
    where
        'a: 'query,
    {
        let index = self.references.and_then(|cache| {
            cache.get().or_else(|| {
                (!self.document.connections.is_empty())
                    .then(|| cache.get_or_init(|| PortReferenceIndex::new(self.document)))
            })
        });
        let indexed = index
            .and_then(|index| index.connections_for(address))
            .into_iter()
            .flatten()
            .map(move |id| &self.document.connections[id]);
        let scanned = index
            .is_none()
            .then(|| self.document.connections.values())
            .into_iter()
            .flatten()
            .filter(move |connection| {
                connection.output == *address || connection.input == *address
            });
        indexed.chain(scanned)
    }

    pub fn unreferenced_derived_bindings(
        self,
    ) -> impl Iterator<Item = (&'a PortAddress, &'a DynamicPortBinding)> + 'a {
        let index = self.references.and_then(|cache| {
            cache.get().or_else(|| {
                self.document
                    .port_bindings
                    .values()
                    .any(|binding| !matches!(binding, DynamicPortBinding::UserCreated { .. }))
                    .then(|| cache.get_or_init(|| PortReferenceIndex::new(self.document)))
            })
        });
        let indexed = index
            .into_iter()
            .flat_map(|index| index.unreferenced_bindings())
            .map(move |address| (address, &self.document.port_bindings[address]));
        let scanned = self
            .references
            .is_none()
            .then(|| self.document.port_bindings.iter())
            .into_iter()
            .flatten()
            .filter(move |(address, binding)| {
                !matches!(binding, DynamicPortBinding::UserCreated { .. })
                    && !self.document.input_states.contains_key(*address)
                    && !self.document.connections.values().any(|connection| {
                        connection.output == **address || connection.input == **address
                    })
            });
        indexed.chain(scanned)
    }

    pub fn validate_connection_candidate(
        self,
        removals: &BTreeMap<ConnectionId, DocumentConnection>,
        insertions: &BTreeMap<ConnectionId, DocumentConnection>,
    ) -> Result<(), DocumentError> {
        validate_connection_candidate(self.document, removals, insertions, self.validated)
    }

    pub(crate) fn from_validity(
        document: &'a GraphDocument,
        validated: bool,
        references: Option<&'a OnceLock<PortReferenceIndex>>,
    ) -> Self {
        Self {
            document,
            validated,
            references,
        }
    }

    pub(crate) fn is_validated(self) -> bool {
        self.validated
    }
}

fn validate_connection_candidate(
    document: &GraphDocument,
    removals: &BTreeMap<ConnectionId, DocumentConnection>,
    insertions: &BTreeMap<ConnectionId, DocumentConnection>,
    validated: bool,
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
    if validated {
        for (id, connection) in insertions {
            validate_connection(document, id, connection)?;
        }
        return Ok(());
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

pub(crate) fn validate_graph_document_patch(
    document: &GraphDocument,
    operations: &[GraphDocumentOperation],
    validated: bool,
) -> Result<(), DocumentError> {
    // Destructive changes can invalidate retained references. Invalid source documents
    // must also be checked in full, since a patch may repair their original defects.
    if !validated
        || operations.iter().any(|operation| match operation {
            GraphDocumentOperation::RemoveNode { node } => !document.nodes.contains_key(&node.id),
            GraphDocumentOperation::RemovePortBinding { address, .. } => {
                !document.port_bindings.contains_key(address)
            }
            _ => false,
        })
    {
        return validate_graph_document(document);
    }
    let mut constants_changed = false;
    let mut bindings = BTreeSet::new();
    let mut input_states = BTreeSet::new();
    let mut connections = BTreeSet::new();
    for operation in operations {
        match operation {
            GraphDocumentOperation::SetConstant { .. } => constants_changed = true,
            GraphDocumentOperation::InsertPortBinding { address, .. } => {
                bindings.insert(address);
            }
            GraphDocumentOperation::SetInputState { address, .. } => {
                input_states.insert(address);
            }
            GraphDocumentOperation::InsertConnection { connection } => {
                connections.insert(connection.id);
            }
            // Successful node operations already preserve their map key and identity.
            GraphDocumentOperation::InsertNode { .. }
            | GraphDocumentOperation::UpdateNode { .. }
            | GraphDocumentOperation::RemoveNode { .. }
            | GraphDocumentOperation::RemovePortBinding { .. }
            | GraphDocumentOperation::RemoveConnection { .. } => {}
        }
    }
    // Keep the full validator's category and key order when several changes are invalid.
    if constants_changed {
        validate_constants(document)?;
    }
    for address in bindings {
        if document.port_bindings.contains_key(address) {
            validate_binding(document, address)?;
        }
    }
    for address in input_states {
        if document.input_states.contains_key(address) {
            validate_address(document, address)?;
        }
    }
    for id in connections {
        if let Some(connection) = document.connections.get(&id) {
            validate_connection(document, &id, connection)?;
        }
    }
    Ok(())
}

fn validate_document_with_connections<'a>(
    document: &GraphDocument,
    connections: impl Iterator<Item = (&'a ConnectionId, &'a DocumentConnection)>,
) -> Result<(), DocumentError> {
    validate_constants(document)?;
    for (id, node) in &document.nodes {
        if id != &node.id {
            return Err(DocumentError::DuplicateNode(node.id));
        }
    }
    for address in document.port_bindings.keys() {
        validate_binding(document, address)?;
    }
    for address in document.input_states.keys() {
        validate_address(document, address)?;
    }
    for (id, connection) in connections {
        validate_connection(document, id, connection)?;
    }
    Ok(())
}

fn validate_constants(document: &GraphDocument) -> Result<(), DocumentError> {
    yss_graph_document::validate_constant_definitions(&document.constants)
        .map_err(|error| DocumentError::InvalidConstant(error.id))
}

fn validate_binding(document: &GraphDocument, address: &PortAddress) -> Result<(), DocumentError> {
    validate_endpoint(document, address)?;
    if !address.is_instance() {
        return Err(DocumentError::UnexpectedPortBinding(address.clone()));
    }
    Ok(())
}

fn validate_connection(
    document: &GraphDocument,
    id: &ConnectionId,
    connection: &DocumentConnection,
) -> Result<(), DocumentError> {
    if id != &connection.id {
        return Err(DocumentError::DuplicateConnection(connection.id));
    }
    validate_address(document, &connection.output)?;
    validate_address(document, &connection.input)
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
