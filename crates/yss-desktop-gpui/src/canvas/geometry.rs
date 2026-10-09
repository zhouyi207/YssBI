use std::collections::BTreeMap;

use gpui::{Bounds, Pixels, Point, point, px, size};
use yss_graph_document::{NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::{EditorNodeModel, EditorParameterModel};
use yss_node_protocol::{ParameterPresentation, PortDirection};

pub const NODE_WIDTH: f32 = 280.;
pub const TITLE_HEIGHT: f32 = 44.;
pub const PORT_HEIGHT: f32 = 28.;
pub const PARAMETER_HEIGHT: f32 = 24.;

#[derive(Clone, Copy)]
pub struct NodeLayout {
    pub compact: bool,
    pub width: f32,
    pub height: f32,
    pub ports_top: f32,
    pub port_height: f32,
}

impl NodeLayout {
    pub fn new(node: &EditorNodeModel) -> Self {
        let inputs = node
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .count();
        let rows = inputs.max(node.ports.len() - inputs).max(1) as f32;
        if node.display.style_id.as_deref() == Some("builtin.reroute") {
            return Self {
                compact: true,
                width: 32.,
                height: rows * 20.,
                ports_top: 0.,
                port_height: 20.,
            };
        }
        let parameters = inline_parameters(node).count();
        let ports_top = TITLE_HEIGHT
            + if parameters == 0 {
                0.
            } else {
                parameters as f32 * PARAMETER_HEIGHT + 12.
            };
        Self {
            compact: false,
            width: NODE_WIDTH,
            height: ports_top + rows * PORT_HEIGHT + 12.,
            ports_top,
            port_height: PORT_HEIGHT,
        }
    }
}

pub fn inline_parameters(node: &EditorNodeModel) -> impl Iterator<Item = &EditorParameterModel> {
    node.parameter_groups
        .iter()
        .flat_map(|group| group.parameters.iter())
        .filter(|parameter| parameter.presentation == ParameterPresentation::InlineAndDetail)
}

pub fn position(node: &EditorNodeModel, preview: &BTreeMap<NodeId, NodePosition>) -> NodePosition {
    preview.get(&node.node_id).copied().unwrap_or(node.position)
}

pub fn node_bounds(node: &EditorNodeModel, position: NodePosition) -> Bounds<Pixels> {
    let layout = NodeLayout::new(node);
    Bounds::new(
        point(px(position.x as f32), px(position.y as f32)),
        size(px(layout.width), px(layout.height)),
    )
}

pub fn port_offsets(node: &EditorNodeModel) -> impl Iterator<Item = (PortAddress, Point<Pixels>)> {
    let mut inputs = 0;
    let mut outputs = 0;
    let layout = NodeLayout::new(node);
    node.ports.iter().map(move |port| {
        let output = port.direction == PortDirection::Output;
        let index = if output { &mut outputs } else { &mut inputs };
        let offset = point(
            px(if output { layout.width } else { 0. }),
            px(layout.ports_top + (*index as f32 + 0.5) * layout.port_height),
        );
        *index += 1;
        (port.address.clone(), offset)
    })
}
