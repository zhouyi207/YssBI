use std::fmt;
use yss_graph_document::{ConnectionId, NodeId, PortAddress};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    DuplicateNode(NodeId),
    NodeNotFound(NodeId),
    DuplicateConnection(ConnectionId),
    ConnectionNotFound(ConnectionId),
    EndpointNodeNotFound(NodeId),
    MissingPortBinding(PortAddress),
    UnexpectedPortBinding(PortAddress),
    DuplicatePortBinding(PortAddress),
    PortBindingNotFound(PortAddress),
    InputStateMismatch(PortAddress),
    NodeContentMismatch(NodeId),
    NodeIdentityMismatch { before: NodeId, after: NodeId },
    ConnectionContentMismatch(ConnectionId),
    PortBindingContentMismatch(PortAddress),
    RevisionExhausted { retained: u64 },
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(formatter, "node '{id}' already exists"),
            Self::NodeNotFound(id) => write!(formatter, "node '{id}' does not exist"),
            Self::DuplicateConnection(id) => {
                write!(formatter, "connection '{id}' already exists")
            }
            Self::ConnectionNotFound(id) => {
                write!(formatter, "connection '{id}' does not exist")
            }
            Self::EndpointNodeNotFound(id) => {
                write!(formatter, "endpoint node '{id}' does not exist")
            }
            Self::MissingPortBinding(address) => {
                write!(formatter, "instance port '{address}' has no binding")
            }
            Self::UnexpectedPortBinding(address) => {
                write!(formatter, "declared port '{address}' cannot have a binding")
            }
            Self::DuplicatePortBinding(address) => {
                write!(formatter, "port binding '{address}' already exists")
            }
            Self::PortBindingNotFound(address) => {
                write!(formatter, "port binding '{address}' does not exist")
            }
            Self::InputStateMismatch(address) => {
                write!(
                    formatter,
                    "input state '{address}' does not match patch precondition"
                )
            }
            Self::NodeContentMismatch(id) => {
                write!(formatter, "node '{id}' does not match patch precondition")
            }
            Self::NodeIdentityMismatch { before, after } => write!(
                formatter,
                "node patch cannot change identity from '{before}' to '{after}'"
            ),
            Self::ConnectionContentMismatch(id) => {
                write!(
                    formatter,
                    "connection '{id}' does not match patch precondition"
                )
            }
            Self::PortBindingContentMismatch(address) => {
                write!(
                    formatter,
                    "port binding '{address}' does not match patch precondition"
                )
            }
            Self::RevisionExhausted { retained } => {
                write!(formatter, "document revision is exhausted at {retained}")
            }
        }
    }
}

impl std::error::Error for DocumentError {}
