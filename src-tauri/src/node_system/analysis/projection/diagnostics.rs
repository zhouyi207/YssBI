use super::super::{DiagnosticLocation, DiagnosticSeverity, NodeDiagnostic};
use super::project_address;
use super::types::{
    DiagnosticDto, DiagnosticLocationDto, DiagnosticSeverityDto, LocalizationLookup,
};
use crate::graph_document::{ConnectionId, GraphDocument, NodeId, PortAddress};

type EditorDiagnostic = NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

pub(super) fn project_diagnostic(
    diagnostic: &EditorDiagnostic,
    localization: &impl LocalizationLookup,
) -> DiagnosticDto {
    DiagnosticDto {
        code: diagnostic.code.as_str().into(),
        message: localization.text(&diagnostic.message_key, &diagnostic.arguments),
        severity: diagnostic.severity.into(),
        blocking: diagnostic.severity.is_blocking(),
        location: project_location(&diagnostic.primary),
        related: diagnostic.related.iter().map(project_location).collect(),
    }
}

fn project_location(
    location: &DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
) -> DiagnosticLocationDto {
    match location {
        DiagnosticLocation::Graph => DiagnosticLocationDto::Graph,
        DiagnosticLocation::Node(node_id) => DiagnosticLocationDto::Node {
            node_id: node_id.to_string().into(),
        },
        DiagnosticLocation::Port(address) => DiagnosticLocationDto::Port {
            address: project_address(address),
        },
        DiagnosticLocation::Connection(connection_id) => DiagnosticLocationDto::Connection {
            connection_id: connection_id.to_string().into(),
        },
        DiagnosticLocation::Parameter { node_id, key } => DiagnosticLocationDto::Parameter {
            node_id: node_id.to_string().into(),
            key: key.as_str().into(),
        },
        DiagnosticLocation::Resource(identity) => DiagnosticLocationDto::Resource {
            identity: identity.clone(),
        },
    }
}

pub(super) fn diagnostic_belongs_to_node(
    diagnostic: &EditorDiagnostic,
    node_id: NodeId,
    document: &GraphDocument,
) -> bool {
    match &diagnostic.primary {
        DiagnosticLocation::Node(id) | DiagnosticLocation::Parameter { node_id: id, .. } => {
            *id == node_id
        }
        DiagnosticLocation::Port(address) => address.node_id == node_id,
        DiagnosticLocation::Connection(connection_id) => document
            .connections
            .get(connection_id)
            .is_some_and(|connection| {
                connection.input.node_id == node_id || connection.output.node_id == node_id
            }),
        DiagnosticLocation::Graph | DiagnosticLocation::Resource(_) => false,
    }
}
impl From<DiagnosticSeverity> for DiagnosticSeverityDto {
    fn from(value: DiagnosticSeverity) -> Self {
        match value {
            DiagnosticSeverity::Error => Self::Error,
            DiagnosticSeverity::Warning => Self::Warning,
            DiagnosticSeverity::Information => Self::Information,
        }
    }
}
