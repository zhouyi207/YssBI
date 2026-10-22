mod curve;
mod drag;
mod interaction;
mod menu;
mod render;

pub(super) use curve::Tessellator;
pub(super) use drag::ConnectionDrag;
pub(super) use interaction::ConnectionClick;

use gpui_kit::{Bounds, Hsla, Path, Pixels, Point, Window, point, px};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use yss_graph_document::{ConnectionId, NodeId, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_node_protocol::PortDirection;

use super::geometry::NodeMovement;
use super::presentation::{Presentation, State};
use super::style::{CanvasStyle, DataKind};
use super::{geometry, ports};
use curve::{Curve, Dash};

/// The pointer tolerance around a connection, in screen pixels.
const HIT_DISTANCE: f32 = 6.;

#[derive(Clone)]
struct PortAnchor {
    node_id: NodeId,
    node_position: Point<Pixels>,
    offset: Point<Pixels>,
    key: Arc<str>,
    direction: PortDirection,
    kind: DataKind,
}

impl PortAnchor {
    fn position(&self, preview: &NodeMovement) -> Point<Pixels> {
        self.offset
            + preview
                .position(self.node_id)
                .map_or(self.node_position, |position| {
                    point(px(position.x as f32), px(position.y as f32))
                })
    }
}

struct Connection {
    id: ConnectionId,
    output: PortAnchor,
    input: PortAnchor,
}

impl Connection {
    fn geometry(
        &self,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &NodeMovement,
    ) -> (Point<Pixels>, Curve) {
        let output = self.output.position(preview) * zoom;
        (
            offset + output,
            Curve::new(
                self.input.position(preview) * zoom - output,
                zoom,
                self.output.direction == PortDirection::Input,
            ),
        )
    }
}

pub(super) struct Interaction<'a> {
    pub presentation: &'a Presentation,
    pub selected: &'a BTreeSet<ConnectionId>,
    pub hovered: Option<ConnectionId>,
    pub replacements: Option<&'a BTreeSet<ConnectionId>>,
}

/// The stroke width of a connection: it thickens gently with zoom but stays crisp.
pub(super) fn stroke_width(zoom: f32) -> Pixels {
    px((2. * zoom.sqrt()).clamp(1.25, 3.))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pass {
    Line,
    /// Hovered and selected connections stay above the rest.
    Emphasis,
}

struct StrokeGroup {
    pass: Pass,
    color: Hsla,
    width: Pixels,
    dash: Option<Dash>,
    path: Option<Path<Pixels>>,
}

/// Strokes merged per appearance, so a frame paints a handful of paths however many
/// connections are visible.
#[derive(Default)]
struct Strokes {
    tessellator: Tessellator,
    groups: Vec<StrokeGroup>,
}

impl Strokes {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        pass: Pass,
        color: Hsla,
        width: Pixels,
        dash: Option<Dash>,
        origin: Point<Pixels>,
        curve: Curve,
        hidden: &dyn Fn(Point<Pixels>) -> bool,
    ) {
        if color.a <= 0. {
            return;
        }
        let index = match self.groups.iter().position(|group| {
            group.pass == pass && group.color == color && group.width == width && group.dash == dash
        }) {
            Some(index) => index,
            None => {
                self.groups.push(StrokeGroup {
                    pass,
                    color,
                    width,
                    dash,
                    path: None,
                });
                self.groups.len() - 1
            }
        };
        let path = &mut self.groups[index].path;
        self.tessellator
            .stroke(curve, origin, width, dash, hidden, path);
    }

    fn paint(mut self, window: &mut Window) {
        // Emphasized lines go above every other; within a pass, first use keeps its place.
        self.groups.sort_by_key(|group| group.pass);
        for group in self.groups {
            if let Some(path) = group.path {
                window.paint_path(path, group.color);
            }
        }
    }
}

fn shown(_: Point<Pixels>) -> bool {
    false
}

pub(super) struct ConnectionLayer {
    nodes: Vec<geometry::NodeGeometry>,
    node_indices: BTreeMap<NodeId, usize>,
    anchors: BTreeMap<PortAddress, PortAnchor>,
    connections: Vec<Connection>,
}

impl ConnectionLayer {
    pub fn new(projection: &EditorProjectionModel) -> Self {
        let mut layer = Self {
            nodes: Vec::new(),
            node_indices: BTreeMap::new(),
            anchors: BTreeMap::new(),
            connections: Vec::new(),
        };
        layer.install(projection);
        layer
    }

    pub fn install(&mut self, projection: &EditorProjectionModel) {
        let mut nodes = self
            .nodes
            .drain(..)
            .map(|geometry| (geometry.node_id, geometry))
            .collect::<BTreeMap<_, _>>();
        self.nodes.reserve(projection.nodes.len());
        self.node_indices.clear();
        let mut anchors = std::mem::take(&mut self.anchors);
        for node in &projection.nodes {
            let retained = nodes
                .remove(&node.node_id)
                .filter(|geometry| geometry.matches(node));
            let geometry = retained.unwrap_or_else(|| geometry::NodeGeometry::new(node));
            for (output, indices) in [(false, &geometry.inputs), (true, &geometry.outputs)] {
                for (row, index) in indices.iter().enumerate() {
                    let port = &node.ports[*index];
                    let offset = geometry.layout.port_offset(output, row);
                    let mut anchor = anchors.remove(&port.address).unwrap_or_else(|| PortAnchor {
                        node_id: node.node_id,
                        node_position: Point::default(),
                        offset,
                        key: port.address.to_string().into(),
                        direction: port.direction,
                        kind: ports::data_kind(port),
                    });
                    anchor.node_position =
                        point(px(node.position.x as f32), px(node.position.y as f32));
                    anchor.offset = offset;
                    anchor.direction = port.direction;
                    anchor.kind = ports::data_kind(port);
                    self.anchors.insert(port.address.clone(), anchor);
                }
            }
            self.node_indices.insert(node.node_id, self.nodes.len());
            self.nodes.push(geometry);
        }
        self.connections = projection
            .connections
            .iter()
            .filter_map(|connection| {
                Some(Connection {
                    id: connection.connection_id,
                    output: self.anchors.get(&connection.output)?.clone(),
                    input: self.anchors.get(&connection.input)?.clone(),
                })
            })
            .collect();
    }

    pub fn node(&self, id: NodeId) -> Option<&geometry::NodeGeometry> {
        self.nodes.get(*self.node_indices.get(&id)?)
    }

    pub fn nodes(&self) -> &[geometry::NodeGeometry] {
        &self.nodes
    }

    pub fn port_position(
        &self,
        address: &PortAddress,
        preview: &NodeMovement,
    ) -> Option<Point<Pixels>> {
        self.anchors
            .get(address)
            .map(|anchor| anchor.position(preview))
    }

    pub fn port_direction(&self, address: &PortAddress) -> Option<PortDirection> {
        self.anchors.get(address).map(|anchor| anchor.direction)
    }

    pub fn addresses(&self) -> impl Iterator<Item = (&str, &PortAddress)> {
        self.anchors
            .iter()
            .map(|(address, anchor)| (anchor.key.as_ref(), address))
    }

    pub fn paint(
        &self,
        bounds: Bounds<Pixels>,
        (offset, zoom): (Point<Pixels>, f32),
        preview: &NodeMovement,
        interaction: Interaction<'_>,
        style: &CanvasStyle,
        window: &mut Window,
    ) {
        let width = stroke_width(zoom);
        let replaced_dash = Some(Dash {
            on: 6.,
            off: 4.,
            phase: 0.,
        });
        let stale_dash = Some(Dash {
            on: 8.,
            off: 5.,
            phase: 0.,
        });
        let mut strokes = Strokes::default();
        for connection in &self.connections {
            let (origin, curve) = connection.geometry(bounds.origin + offset, zoom, preview);
            if !curve.visible(origin, bounds) {
                continue;
            }
            let state = interaction.presentation.connection(connection.id);
            let selected = interaction.selected.contains(&connection.id);
            let hovered = interaction.hovered == Some(connection.id);
            let replaced = interaction
                .replacements
                .is_some_and(|ids| ids.contains(&connection.id));
            let fade = if interaction.replacements.is_some() && !replaced {
                0.25
            } else {
                1.
            };
            let ink = style.data(connection.output.kind);
            let (color, dash) = if replaced {
                (style.warning, replaced_dash)
            } else if selected {
                (style.selection, None)
            } else {
                match state {
                    State::Error => (style.danger, None),
                    State::Stale => (ink.opacity(0.8), stale_dash),
                    State::Unexecuted => (ink.opacity(0.6), None),
                    // The activity overlay runs bright pulses along the dimmed line.
                    State::Running => (ink.opacity(0.4), None),
                    State::Valid | State::Partial => (ink, None),
                }
            };
            // Emphasis is weight, not glow: the line turns opaque, heavier and comes forward.
            let (color, width, pass) = if (selected || hovered) && !replaced {
                (color.alpha(1.), width + px(1.5), Pass::Emphasis)
            } else {
                (color, width, Pass::Line)
            };
            strokes.add(
                pass,
                color.opacity(fade),
                width,
                dash,
                origin,
                curve,
                &shown,
            );
        }
        strokes.paint(window);
    }

    pub fn hit_test(
        &self,
        target: Point<Pixels>,
        zoom: f32,
        preview: &NodeMovement,
    ) -> Option<ConnectionId> {
        let mut tessellator = Tessellator::default();
        self.connections.iter().rev().find_map(|connection| {
            let (origin, curve) = connection.geometry(Point::default(), zoom, preview);
            let relative = target - origin;
            (curve.bounds().dilate(px(HIT_DISTANCE)).contains(&relative)
                && tessellator.distance(curve, relative) <= px(HIT_DISTANCE))
            .then_some(connection.id)
        })
    }

    pub fn has_visible_running(
        &self,
        presentation: &Presentation,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &NodeMovement,
    ) -> bool {
        self.connections
            .iter()
            .filter(|connection| presentation.connection(connection.id) == State::Running)
            .any(|connection| {
                let (origin, curve) = connection.geometry(bounds.origin + offset, zoom, preview);
                curve.visible(origin, bounds)
            })
    }

    /// Pulses travelling from output to input along running connections. They are drawn
    /// above the graph, so stretches passing under a node are left out.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_activity(
        &self,
        bounds: Bounds<Pixels>,
        (offset, zoom): (Point<Pixels>, f32),
        preview: &NodeMovement,
        (presentation, travelled): (&Presentation, f32),
        occluders: &[Bounds<Pixels>],
        (style, fade): (&CanvasStyle, f32),
        window: &mut Window,
    ) {
        let width = stroke_width(zoom) + px(0.5);
        let mut strokes = Strokes::default();
        for connection in self
            .connections
            .iter()
            .filter(|connection| presentation.connection(connection.id) == State::Running)
        {
            let (origin, curve) = connection.geometry(bounds.origin + offset, zoom, preview);
            if !curve.visible(origin, bounds) {
                continue;
            }
            let mut extent = curve.bounds();
            extent.origin += origin;
            let covering = occluders
                .iter()
                .filter(|occluder| occluder.intersects(&extent))
                .collect::<Vec<_>>();
            let hidden =
                |point: Point<Pixels>| covering.iter().any(|occluder| occluder.contains(&point));
            strokes.add(
                Pass::Line,
                style.data(connection.output.kind).opacity(fade),
                width,
                Some(Dash {
                    on: 10.,
                    off: 14.,
                    phase: -travelled,
                }),
                origin,
                curve,
                &hidden,
            );
        }
        strokes.paint(window);
    }

    pub fn paint_pending(
        &self,
        (start, end): (Point<Pixels>, Point<Pixels>),
        (from_input, zoom): (bool, f32),
        color: Hsla,
        window: &mut Window,
    ) {
        let mut strokes = Strokes::default();
        let curve = Curve::new(end - start, zoom, from_input);
        strokes.add(Pass::Line, color, px(2.), None, start, curve, &shown);
        strokes.paint(window);
        let dot = px(7.);
        window.paint_quad(
            gpui_kit::fill(
                Bounds::new(end - point(dot / 2., dot / 2.), gpui_kit::size(dot, dot)),
                color,
            )
            .corner_radii(dot / 2.),
        );
    }
}
