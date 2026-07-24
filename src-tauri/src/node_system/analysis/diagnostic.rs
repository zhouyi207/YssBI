use crate::node_system::protocol::{I18nKey, ParameterKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiagnosticCode(Box<str>);

impl DiagnosticCode {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub type DiagnosticArguments = BTreeMap<Box<str>, Box<str>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
}

impl DiagnosticSeverity {
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::Error)
    }
}

pub type Severity = DiagnosticSeverity;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLocation<NodeId, PortAddress, ConnectionId, ResourceIdentity> {
    Graph,
    Node(NodeId),
    Port(PortAddress),
    Connection(ConnectionId),
    Parameter { node_id: NodeId, key: ParameterKey },
    Resource(ResourceIdentity),
}

pub type Location<NodeId, PortAddress, ConnectionId, ResourceIdentity> =
    DiagnosticLocation<NodeId, PortAddress, ConnectionId, ResourceIdentity>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDiagnostic<NodeId, PortAddress, ConnectionId, ResourceIdentity> {
    pub code: DiagnosticCode,
    pub message_key: I18nKey,
    pub arguments: DiagnosticArguments,
    pub severity: DiagnosticSeverity,
    pub primary: DiagnosticLocation<NodeId, PortAddress, ConnectionId, ResourceIdentity>,
    pub related: Box<[DiagnosticLocation<NodeId, PortAddress, ConnectionId, ResourceIdentity>]>,
}
