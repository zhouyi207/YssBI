use super::{
    DocumentConnection, DocumentError, DocumentNode, DynamicPortBinding, GraphDocument, InputState,
    PortAddress,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphDocumentPatch {
    pub operations: Vec<GraphDocumentOperation>,
}

impl GraphDocumentPatch {
    pub fn new(operations: impl Into<Vec<GraphDocumentOperation>>) -> Self {
        Self {
            operations: operations.into(),
        }
    }

    pub fn inverse(&self) -> Self {
        Self {
            operations: self
                .operations
                .iter()
                .rev()
                .map(GraphDocumentOperation::inverse)
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub(crate) fn apply_without_revision(
        &self,
        document: &mut GraphDocument,
    ) -> Result<(), DocumentError> {
        for operation in &self.operations {
            operation.apply(document)?;
        }
        document.validate()
    }
}

impl From<Vec<GraphDocumentOperation>> for GraphDocumentPatch {
    fn from(operations: Vec<GraphDocumentOperation>) -> Self {
        Self::new(operations)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GraphDocumentOperation {
    InsertNode {
        node: DocumentNode,
    },
    RemoveNode {
        node: DocumentNode,
    },
    UpdateNode {
        before: DocumentNode,
        after: DocumentNode,
    },
    InsertPortBinding {
        address: PortAddress,
        binding: DynamicPortBinding,
    },
    RemovePortBinding {
        address: PortAddress,
        binding: DynamicPortBinding,
    },
    InsertConnection {
        connection: DocumentConnection,
    },
    RemoveConnection {
        connection: DocumentConnection,
    },
    SetInputState {
        address: PortAddress,
        before: Option<InputState>,
        after: Option<InputState>,
    },
}

impl GraphDocumentOperation {
    pub fn inverse(&self) -> Self {
        match self {
            Self::InsertNode { node } => Self::RemoveNode { node: node.clone() },
            Self::RemoveNode { node } => Self::InsertNode { node: node.clone() },
            Self::UpdateNode { before, after } => Self::UpdateNode {
                before: after.clone(),
                after: before.clone(),
            },
            Self::InsertPortBinding { address, binding } => Self::RemovePortBinding {
                address: address.clone(),
                binding: binding.clone(),
            },
            Self::RemovePortBinding { address, binding } => Self::InsertPortBinding {
                address: address.clone(),
                binding: binding.clone(),
            },
            Self::InsertConnection { connection } => Self::RemoveConnection {
                connection: connection.clone(),
            },
            Self::RemoveConnection { connection } => Self::InsertConnection {
                connection: connection.clone(),
            },
            Self::SetInputState {
                address,
                before,
                after,
            } => Self::SetInputState {
                address: address.clone(),
                before: after.clone(),
                after: before.clone(),
            },
        }
    }

    fn apply(&self, document: &mut GraphDocument) -> Result<(), DocumentError> {
        match self {
            Self::InsertNode { node } => insert_node(document, node),
            Self::RemoveNode { node } => remove_node(document, node),
            Self::UpdateNode { before, after } => update_node(document, before, after),
            Self::InsertPortBinding { address, binding } => {
                insert_port_binding(document, address, binding)
            }
            Self::RemovePortBinding { address, binding } => {
                remove_port_binding(document, address, binding)
            }
            Self::InsertConnection { connection } => insert_connection(document, connection),
            Self::RemoveConnection { connection } => remove_connection(document, connection),
            Self::SetInputState {
                address,
                before,
                after,
            } => set_input_state(document, address, before, after),
        }
    }
}

fn insert_node(document: &mut GraphDocument, node: &DocumentNode) -> Result<(), DocumentError> {
    if document.nodes.contains_key(&node.id) {
        return Err(DocumentError::DuplicateNode(node.id));
    }
    document.nodes.insert(node.id, node.clone());
    Ok(())
}

fn remove_node(document: &mut GraphDocument, node: &DocumentNode) -> Result<(), DocumentError> {
    match document.nodes.get(&node.id) {
        None => Err(DocumentError::NodeNotFound(node.id)),
        Some(current) if current != node => Err(DocumentError::NodeContentMismatch(node.id)),
        Some(_) => {
            document.nodes.remove(&node.id);
            Ok(())
        }
    }
}

fn update_node(
    document: &mut GraphDocument,
    before: &DocumentNode,
    after: &DocumentNode,
) -> Result<(), DocumentError> {
    if before.id != after.id {
        return Err(DocumentError::NodeIdentityMismatch {
            before: before.id,
            after: after.id,
        });
    }
    match document.nodes.get(&before.id) {
        None => Err(DocumentError::NodeNotFound(before.id)),
        Some(current) if current != before => Err(DocumentError::NodeContentMismatch(before.id)),
        Some(_) => {
            document.nodes.insert(after.id, after.clone());
            Ok(())
        }
    }
}

fn insert_port_binding(
    document: &mut GraphDocument,
    address: &PortAddress,
    binding: &DynamicPortBinding,
) -> Result<(), DocumentError> {
    if document.port_bindings.contains_key(address) {
        return Err(DocumentError::DuplicatePortBinding(address.clone()));
    }
    document
        .port_bindings
        .insert(address.clone(), binding.clone());
    Ok(())
}

fn remove_port_binding(
    document: &mut GraphDocument,
    address: &PortAddress,
    binding: &DynamicPortBinding,
) -> Result<(), DocumentError> {
    match document.port_bindings.get(address) {
        None => Err(DocumentError::PortBindingNotFound(address.clone())),
        Some(current) if current != binding => {
            Err(DocumentError::PortBindingContentMismatch(address.clone()))
        }
        Some(_) => {
            document.port_bindings.remove(address);
            Ok(())
        }
    }
}

fn insert_connection(
    document: &mut GraphDocument,
    connection: &DocumentConnection,
) -> Result<(), DocumentError> {
    if document.connections.contains_key(&connection.id) {
        return Err(DocumentError::DuplicateConnection(connection.id));
    }
    document
        .connections
        .insert(connection.id, connection.clone());
    Ok(())
}

fn remove_connection(
    document: &mut GraphDocument,
    connection: &DocumentConnection,
) -> Result<(), DocumentError> {
    match document.connections.get(&connection.id) {
        None => Err(DocumentError::ConnectionNotFound(connection.id)),
        Some(current) if current != connection => {
            Err(DocumentError::ConnectionContentMismatch(connection.id))
        }
        Some(_) => {
            document.connections.remove(&connection.id);
            Ok(())
        }
    }
}

fn set_input_state(
    document: &mut GraphDocument,
    address: &PortAddress,
    before: &Option<InputState>,
    after: &Option<InputState>,
) -> Result<(), DocumentError> {
    if document.input_states.get(address) != before.as_ref() {
        return Err(DocumentError::InputStateMismatch(address.clone()));
    }
    match after {
        Some(state) => {
            document.input_states.insert(address.clone(), state.clone());
        }
        None => {
            document.input_states.remove(address);
        }
    }
    Ok(())
}

impl GraphDocument {
    pub fn apply_patch(&mut self, patch: &GraphDocumentPatch) -> Result<(), DocumentError> {
        let next_revision =
            self.revision
                .checked_next()
                .map_err(|error| DocumentError::RevisionExhausted {
                    retained: error.retained,
                })?;
        let mut staged = self.clone();
        patch.apply_without_revision(&mut staged)?;
        staged.revision = next_revision;
        *self = staged;
        Ok(())
    }
}
