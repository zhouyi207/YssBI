use std::{collections::BTreeMap, rc::Rc};

use gpui_kit::{Bounds, Pixels, Point, point, px, size};
use yss_graph_document::{NodeId, NodePosition};
use yss_graph_editor::NodePositionMutation;
use yss_graph_editor::projection::{EditorNodeModel, EditorParameterModel};
use yss_node_protocol::{ParameterPresentation, PortDirection};

// Paint snapshots share the press-time positions; displacement stays local to each frame.
#[derive(Clone, Default)]
pub struct NodeMovement {
    originals: Option<Rc<BTreeMap<NodeId, NodePosition>>>,
    displacement: Point<Pixels>,
}

impl NodeMovement {
    pub fn new(positions: BTreeMap<NodeId, NodePosition>) -> Self {
        Self {
            originals: (!positions.is_empty()).then(|| Rc::new(positions)),
            displacement: Point::default(),
        }
    }

    pub fn set_displacement(&mut self, displacement: Point<Pixels>) {
        self.displacement = displacement;
    }

    pub fn position(&self, id: NodeId) -> Option<NodePosition> {
        self.originals
            .as_ref()?
            .get(&id)
            .map(|position| self.translate(*position))
    }

    fn translate(&self, position: NodePosition) -> NodePosition {
        NodePosition {
            x: position.x + f32::from(self.displacement.x) as f64,
            y: position.y + f32::from(self.displacement.y) as f64,
        }
    }

    pub fn mutations(&self) -> Vec<NodePositionMutation> {
        if self.displacement == Point::default() {
            return Vec::new();
        }
        let Some(originals) = &self.originals else {
            return Vec::new();
        };
        let mut mutations = Vec::with_capacity(originals.len());
        for (node_id, original) in originals.iter() {
            let position = self.translate(*original);
            if position != *original {
                mutations.push(NodePositionMutation {
                    node_id: *node_id,
                    position,
                });
            }
        }
        mutations
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

// These are world units. Canvas font_scale participates in the shared screen transform,
// so font-size/zoom changes move bounds, ports and painted controls together.
pub const NODE_WIDTH: f32 = 280.;
pub const TITLE_HEIGHT: f32 = 44.;
pub const PORT_HEIGHT: f32 = 28.;
pub const PARAMETER_HEIGHT: f32 = 24.;

#[derive(Clone, Copy, Debug, PartialEq)]
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

    pub fn bounds(self, position: NodePosition) -> Bounds<Pixels> {
        Bounds::new(
            point(px(position.x as f32), px(position.y as f32)),
            size(px(self.width), px(self.height)),
        )
    }

    pub fn port_offset(self, output: bool, index: usize) -> Point<Pixels> {
        point(
            px(if output { self.width } else { 0. }),
            px(self.ports_top + (index as f32 + 0.5) * self.port_height),
        )
    }
}

pub fn inline_parameters(node: &EditorNodeModel) -> impl Iterator<Item = &EditorParameterModel> {
    node.parameter_groups
        .iter()
        .flat_map(|group| group.parameters.iter())
        .filter(|parameter| parameter.presentation == ParameterPresentation::InlineAndDetail)
}

pub fn position(node: &EditorNodeModel, preview: &NodeMovement) -> NodePosition {
    preview.position(node.node_id).unwrap_or(node.position)
}

pub(super) struct NodeGeometry {
    pub node_id: NodeId,
    pub element_id: gpui_kit::SharedString,
    pub layout: NodeLayout,
    pub inputs: Box<[usize]>,
    pub outputs: Box<[usize]>,
}

impl NodeGeometry {
    pub fn new(node: &EditorNodeModel) -> Self {
        Self {
            node_id: node.node_id,
            element_id: format!("node-{}", node.node_id).into(),
            layout: NodeLayout::new(node),
            inputs: port_indices(node, PortDirection::Input).collect(),
            outputs: port_indices(node, PortDirection::Output).collect(),
        }
    }

    pub fn matches(&self, node: &EditorNodeModel) -> bool {
        self.layout == NodeLayout::new(node)
            && self
                .inputs
                .iter()
                .copied()
                .eq(port_indices(node, PortDirection::Input))
            && self
                .outputs
                .iter()
                .copied()
                .eq(port_indices(node, PortDirection::Output))
    }
}

fn port_indices(node: &EditorNodeModel, direction: PortDirection) -> impl Iterator<Item = usize> {
    node.ports
        .iter()
        .enumerate()
        .filter_map(move |(index, port)| (port.direction == direction).then_some(index))
}
