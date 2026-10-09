//! Shared display labels for canonical Problems and Details locations.
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_document::{ConnectionId, NodeId, PortAddress};
use yss_graph_editor::projection::{EditorNodeModel, EditorProjectionModel};

pub(super) type Location = DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>;

pub(in crate::workbench) fn label(
    location: &Location,
    projection: &EditorProjectionModel,
    owner: Option<&EditorNodeModel>,
) -> Option<String> {
    let node = |id: NodeId| {
        owner
            .filter(|node| node.node_id == id)
            .or_else(|| projection.nodes.iter().find(|node| node.node_id == id))
    };
    let port = |address: &PortAddress| {
        let node = node(address.node_id)?;
        let port = node.ports.iter().find(|port| port.address == *address);
        Some(match port {
            Some(port) => format!(
                "{} · {}",
                node_title(node),
                port.display
                    .instance_label
                    .as_deref()
                    .unwrap_or(&port.display.label)
            ),
            None => node_title(node).to_owned(),
        })
    };
    match location {
        DiagnosticLocation::Graph => None,
        DiagnosticLocation::Node(id) => node(*id).map(|node| node_title(node).to_owned()),
        DiagnosticLocation::Resource(identity) => Some(identity.to_string()),
        DiagnosticLocation::Port(address) => port(address),
        DiagnosticLocation::Parameter { node_id, key } => node(*node_id).map(|node| {
            let parameter = node
                .parameter_groups
                .iter()
                .flat_map(|group| group.parameters.iter())
                .find(|parameter| parameter.key == *key);
            match parameter {
                Some(parameter) => format!("{} · {}", node_title(node), parameter.display.title),
                None => node_title(node).to_owned(),
            }
        }),
        DiagnosticLocation::Connection(id) => projection
            .connections
            .iter()
            .find(|connection| connection.connection_id == *id)
            .and_then(
                |connection| match (port(&connection.output), port(&connection.input)) {
                    (Some(output), Some(input)) => Some(format!("{output} → {input}")),
                    (output, input) => output.or(input),
                },
            ),
    }
}

pub(in crate::workbench) fn node_title(node: &EditorNodeModel) -> &str {
    node.display
        .user_label
        .as_deref()
        .unwrap_or(&node.display.title)
}
