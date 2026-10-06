//! Bounded initial context from the same captured graph used by targeted reads.
use super::*;
use crate::result_encoding::json_fits_budget;

const MAX_OVERVIEW_BYTES: usize = 32 * 1024;

pub(super) fn project(
    document: &GraphDocument,
    projection: &EditorProjectionModel,
) -> GraphOverview {
    let mut overview = GraphOverview {
        detail: GraphOverviewDetail::Configuration,
        nodes: projection
            .nodes
            .iter()
            .map(|node| {
                let mut summary = node_summary(node, true, false);
                summary.port_templates = None;
                // Resource bindings and other stored configuration may have no GUI
                // editor. Preserve them alongside the resolved visible defaults.
                let parameters = summary.parameters.as_mut().expect("parameters requested");
                if let Some(stored) = document.nodes.get(&node.node_id) {
                    parameters.extend(
                        stored
                            .parameters
                            .iter()
                            .map(|(key, value)| (key.to_string(), Some(value.clone()))),
                    );
                }
                summary
            })
            .collect(),
        connections: projection
            .connections
            .iter()
            .map(|connection| GraphConnectionInspection {
                connection_id: connection.connection_id.to_string(),
                output: inspect_port(&connection.output),
                input: inspect_port(&connection.input),
                order: connection.order.as_deref().map(str::to_owned),
            })
            .collect(),
        literals: projection
            .nodes
            .iter()
            .flat_map(|node| &node.ports)
            .filter_map(|port| {
                port.input
                    .as_ref()
                    .and_then(|input| input.literal_override.as_ref())
                    .map(|value| GraphOverviewLiteral {
                        address: edit_port(&port.address),
                        value: value.clone(),
                    })
            })
            .collect(),
    };
    if json_fits_budget(&overview, MAX_OVERVIEW_BYTES) {
        return overview;
    }
    // Never send a prefix that looks like a complete graph. Each downgrade has
    // explicit semantics, and all retained nodes/edges still come from one read.
    overview.detail = GraphOverviewDetail::Topology;
    for node in &mut overview.nodes {
        node.parameters = None;
    }
    overview.literals.clear();
    if json_fits_budget(&overview, MAX_OVERVIEW_BYTES) {
        return overview;
    }
    overview.detail = GraphOverviewDetail::Counts;
    overview.nodes.clear();
    overview.connections.clear();
    overview
}
