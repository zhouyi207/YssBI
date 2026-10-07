use std::collections::BTreeMap;

use gpui::{Bounds, Pixels, Point, point, px, size};
use yss_graph_document::{NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorNodeModel;
use yss_node_protocol::PortDirection;

pub const NODE_WIDTH: f32 = 280.;
pub const TITLE_HEIGHT: f32 = 44.;
pub const PORT_HEIGHT: f32 = 28.;

pub fn node_height(node: &EditorNodeModel) -> f32 {
    let inputs = node
        .ports
        .iter()
        .filter(|p| p.direction == PortDirection::Input)
        .count();
    let outputs = node.ports.len() - inputs;
    TITLE_HEIGHT + inputs.max(outputs).max(1) as f32 * PORT_HEIGHT + 12.
}

pub fn position(node: &EditorNodeModel, preview: &BTreeMap<NodeId, NodePosition>) -> NodePosition {
    preview.get(&node.node_id).copied().unwrap_or(node.position)
}

pub fn node_bounds(node: &EditorNodeModel, position: NodePosition) -> Bounds<Pixels> {
    Bounds::new(
        point(px(position.x as f32), px(position.y as f32)),
        size(px(NODE_WIDTH), px(node_height(node))),
    )
}

pub fn port_point(
    node: &EditorNodeModel,
    address: &PortAddress,
    position: NodePosition,
) -> Option<Point<Pixels>> {
    let port = node.ports.iter().find(|p| &p.address == address)?;
    let index = node
        .ports
        .iter()
        .filter(|p| p.direction == port.direction)
        .position(|p| &p.address == address)?;
    Some(point(
        px(position.x as f32
            + if port.direction == PortDirection::Output {
                NODE_WIDTH
            } else {
                0.
            }),
        px(position.y as f32 + TITLE_HEIGHT + (index as f32 + 0.5) * PORT_HEIGHT),
    ))
}
