//! Serializable reversible document changes; validation and application live in document-edit.
use crate::{DocumentConnection, DocumentNode, DynamicPortBinding, InputState, PortAddress};
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
}

impl From<Vec<GraphDocumentOperation>> for GraphDocumentPatch {
    fn from(operations: Vec<GraphDocumentOperation>) -> Self {
        Self::new(operations)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GraphDocumentOperation {
    SetConstant {
        id: crate::ConstantId,
        before: Option<Box<crate::GraphConstant>>,
        after: Option<Box<crate::GraphConstant>>,
    },
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
            Self::SetConstant { id, before, after } => Self::SetConstant {
                id: *id,
                before: after.clone(),
                after: before.clone(),
            },
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
}
