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
pub const HEADER_HEIGHT: f32 = 36.;
pub const PORT_HEIGHT: f32 = 26.;
pub const PARAMETER_HEIGHT: f32 = 22.;
const PARAMETER_MARGIN: f32 = 8.;
const PARAMETER_PADDING: f32 = 4.;
/// The gap above the parameter well plus the well's own top and bottom padding.
pub const PARAMETER_CHROME: f32 = PARAMETER_MARGIN + 2. * PARAMETER_PADDING;
const WELL_INSET: f32 = 8.;
const PORTS_TOP_PADDING: f32 = 6.;
const PORTS_BOTTOM_PADDING: f32 = 8.;
/// Leading and trailing padding shared by the header, the parameter well and port labels.
pub const CONTENT_INSET: f32 = 12.;
pub const GLYPH_SIZE: f32 = 10.;
/// Pins stay grabbable a little outside the card edge they straddle.
pub const PORT_REACH: f32 = 10.;
/// A reroute is drawn as a single dot that its wires meet at the center.
pub const REROUTE_DOT: f32 = 12.;
/// The square around a reroute's dot that grabs the node itself; its pins reach out from
/// the sides, so the dot moves like a node body while wires still pull from its edges.
const REROUTE_SIZE: f32 = 20.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeLayout {
    pub compact: bool,
    pub width: f32,
    pub height: f32,
    pub parameters: usize,
    pub ports_top: f32,
    pub port_height: f32,
}

impl NodeLayout {
    pub fn new(node: &EditorNodeModel) -> Self {
        if node.display.style_id.as_deref() == Some("builtin.reroute") {
            return Self {
                compact: true,
                width: REROUTE_SIZE,
                height: REROUTE_SIZE,
                parameters: 0,
                ports_top: 0.,
                port_height: REROUTE_SIZE,
            };
        }
        let inputs = node
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .count();
        let rows = inputs.max(node.ports.len() - inputs).max(1) as f32;
        let parameters = inline_parameters(node).count();
        let ports_top = HEADER_HEIGHT
            + PORTS_TOP_PADDING
            + if parameters == 0 {
                0.
            } else {
                parameters as f32 * PARAMETER_HEIGHT + PARAMETER_CHROME
            };
        Self {
            compact: false,
            width: NODE_WIDTH,
            height: ports_top + rows * PORT_HEIGHT + PORTS_BOTTOM_PADDING,
            parameters,
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
        if self.compact {
            return point(px(self.width / 2.), px(self.height / 2.));
        }
        point(
            px(if output { self.width } else { 0. }),
            px(self.ports_top + (index as f32 + 0.5) * self.port_height),
        )
    }

    /// The well holding inline parameter rows, relative to the node origin.
    pub fn parameter_well(self) -> Option<Bounds<Pixels>> {
        (self.parameters > 0).then(|| {
            Bounds::new(
                point(px(WELL_INSET), px(HEADER_HEIGHT + PARAMETER_MARGIN)),
                size(
                    px(self.width - 2. * WELL_INSET),
                    px(self.parameters as f32 * PARAMETER_HEIGHT + 2. * PARAMETER_PADDING),
                ),
            )
        })
    }

    pub fn parameter_row_top(self, index: usize) -> f32 {
        HEADER_HEIGHT + PARAMETER_MARGIN + PARAMETER_PADDING + index as f32 * PARAMETER_HEIGHT
    }

    /// The pointer region of a port row half, including the reach outside the card. A
    /// compact node keeps its whole box for the node, so its pins only reach outward.
    pub fn port_region(self, output: bool, index: usize) -> Bounds<Pixels> {
        let half = self.width / 2.;
        let top = self.ports_top + index as f32 * self.port_height;
        let (left, right) = match (self.compact, output) {
            (true, true) => (self.width, self.width + PORT_REACH),
            (true, false) => (-PORT_REACH, 0.),
            (false, true) => (half, self.width + PORT_REACH),
            (false, false) => (-PORT_REACH, half),
        };
        Bounds::from_corners(
            point(px(left), px(top)),
            point(px(right), px(top + self.port_height)),
        )
    }
}

/// How much of a node remains legible at a given user zoom. Faded ranges keep
/// zooming continuous instead of popping between levels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub title: f32,
    pub labels: f32,
    pub controls: bool,
    pub shadows: bool,
}

impl Detail {
    pub fn at(scale: f32) -> Self {
        Self {
            title: ramp(scale, 0.36, 0.46),
            labels: ramp(scale, 0.5, 0.6),
            controls: scale >= 0.55,
            shadows: scale >= 0.3,
        }
    }
}

fn ramp(value: f32, start: f32, end: f32) -> f32 {
    let t = ((value - start) / (end - start)).clamp(0., 1.);
    t * t * (3. - 2. * t)
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

/// The origin that centers a new reroute's dot on `center`.
pub fn reroute_position(center: NodePosition) -> NodePosition {
    let half = f64::from(REROUTE_SIZE / 2.);
    NodePosition {
        x: center.x - half,
        y: center.y - half,
    }
}

pub(super) struct NodeGeometry {
    pub node_id: NodeId,
    pub layout: NodeLayout,
    pub inputs: Box<[usize]>,
    pub outputs: Box<[usize]>,
}

impl NodeGeometry {
    pub fn new(node: &EditorNodeModel) -> Self {
        Self {
            node_id: node.node_id,
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
