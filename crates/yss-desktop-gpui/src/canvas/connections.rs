mod curve;
mod drag;
mod hit;
mod interaction;
mod menu;
mod render;

pub(super) use drag::ConnectionDrag;
pub(super) use interaction::ConnectionClick;
pub(super) use menu::ConnectionMenu;

use gpui::{Bounds, PathBuilder, Pixels, Point, Window, fill, point, px, rgb, size};
use gpui_base::plot::{PathCache, ShapeKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use yss_graph_document::{ConnectionId, NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_node_protocol::PortDirection;

use super::presentation::{Presentation, State};
use super::{geometry, ports};
use crate::appearance as theme;
use curve::Curve;

#[derive(Clone)]
struct PortAnchor {
    node_id: NodeId,
    node_position: Point<Pixels>,
    offset: Point<Pixels>,
    key: Arc<str>,
    direction: PortDirection,
    color: u32,
}

impl PortAnchor {
    fn position(&self, preview: &BTreeMap<NodeId, NodePosition>) -> Point<Pixels> {
        self.offset
            + preview
                .get(&self.node_id)
                .map_or(self.node_position, |position| {
                    point(px(position.x as f32), px(position.y as f32))
                })
    }
}

struct Connection {
    id: ConnectionId,
    output: PortAnchor,
    input: PortAnchor,
    path: PathCache,
    highlight: PathCache,
    hit: hit::HitPath,
}

impl Connection {
    fn geometry(
        &self,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &BTreeMap<NodeId, NodePosition>,
    ) -> (Point<Pixels>, Curve) {
        let output = self.output.position(preview) * zoom;
        (
            offset + output,
            Curve {
                delta: self.input.position(preview) * zoom - output,
                from_input: self.output.direction == PortDirection::Input,
            },
        )
    }
}

pub(super) struct Interaction<'a> {
    pub presentation: &'a Presentation,
    pub selected: &'a BTreeSet<ConnectionId>,
    pub hovered: Option<ConnectionId>,
    pub replacements: Option<&'a BTreeSet<ConnectionId>>,
}

pub(super) struct ConnectionLayer {
    anchors: BTreeMap<PortAddress, PortAnchor>,
    connections: Vec<Connection>,
    pending: PathCache,
    grid: PathCache,
}

impl ConnectionLayer {
    pub fn new(projection: &EditorProjectionModel) -> Self {
        let anchors = projection
            .nodes
            .iter()
            .flat_map(|node| {
                geometry::port_offsets(node).zip(node.ports.iter()).map(
                    |((address, offset), port)| {
                        (
                            address.clone(),
                            PortAnchor {
                                node_id: node.node_id,
                                node_position: point(
                                    px(node.position.x as f32),
                                    px(node.position.y as f32),
                                ),
                                offset,
                                key: address.to_string().into(),
                                direction: port.direction,
                                color: ports::type_color(port),
                            },
                        )
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let connections = projection
            .connections
            .iter()
            .filter_map(|connection| {
                let output = anchors.get(&connection.output)?.clone();
                let input = anchors.get(&connection.input)?.clone();
                Some(Connection {
                    id: connection.connection_id,
                    output,
                    input,
                    path: PathCache::default(),
                    highlight: PathCache::default(),
                    hit: hit::HitPath::default(),
                })
            })
            .collect();
        Self {
            anchors,
            connections,
            pending: PathCache::default(),
            grid: PathCache::default(),
        }
    }

    pub fn port_position(
        &self,
        address: &PortAddress,
        preview: &BTreeMap<NodeId, NodePosition>,
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
        &mut self,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &BTreeMap<NodeId, NodePosition>,
        interaction: Interaction<'_>,
        window: &mut Window,
    ) {
        self.paint_grid(bounds, offset, zoom, window);
        for connection in &mut self.connections {
            let (origin, curve) = connection.geometry(bounds.origin + offset, zoom, preview);
            if !visible(curve, origin, bounds) {
                continue;
            }
            let state = interaction.presentation.connection(connection.id);
            let selected = interaction.selected.contains(&connection.id);
            let hovered = interaction.hovered == Some(connection.id);
            let replaced = interaction
                .replacements
                .is_some_and(|ids| ids.contains(&connection.id));
            let opacity = if interaction.replacements.is_some() && !replaced {
                0.25
            } else {
                1.
            };
            if selected || hovered || state == State::Valid {
                let (width, color, alpha) = if selected {
                    (16., theme::BLUE, 0.55)
                } else if hovered {
                    (7., theme::BLUE, 0.35)
                } else {
                    (6., connection.output.color, 0.16)
                };
                paint_connection(
                    &mut connection.highlight,
                    origin,
                    curve,
                    (px(width), State::Valid),
                    rgb(color).opacity(alpha * opacity),
                    window,
                );
            }
            let color = if replaced {
                theme::AMBER
            } else if state == State::Error {
                theme::RED
            } else {
                connection.output.color
            };
            let width = if replaced || state == State::Valid {
                3.
            } else {
                2.
            };
            paint_connection(
                &mut connection.path,
                origin,
                curve,
                (px(width), state),
                rgb(color).opacity(state.opacity() * opacity),
                window,
            );
        }
    }

    pub fn hit_test(
        &mut self,
        point: Point<Pixels>,
        zoom: f32,
        preview: &BTreeMap<NodeId, NodePosition>,
    ) -> Option<ConnectionId> {
        self.connections.iter_mut().rev().find_map(|connection| {
            let (origin, curve) = connection.geometry(Point::default(), zoom, preview);
            connection
                .hit
                .contains(curve, point - origin)
                .then_some(connection.id)
        })
    }

    pub fn paint_pending(
        &mut self,
        a: Point<Pixels>,
        b: Point<Pixels>,
        from_input: bool,
        color: u32,
        window: &mut Window,
    ) {
        paint_connection(
            &mut self.pending,
            a,
            Curve {
                delta: b - a,
                from_input,
            },
            (px(2.), State::Valid),
            rgb(color),
            window,
        );
    }

    pub fn has_visible_running(
        &self,
        presentation: &Presentation,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &BTreeMap<NodeId, NodePosition>,
    ) -> bool {
        self.connections
            .iter()
            .filter(|connection| presentation.connection(connection.id) == State::Running)
            .any(|connection| {
                let (origin, curve) = connection.geometry(bounds.origin + offset, zoom, preview);
                visible(curve, origin, bounds)
            })
    }

    pub fn paint_activity(
        &self,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &BTreeMap<NodeId, NodePosition>,
        (presentation, progress): (&Presentation, f32),
        window: &mut Window,
    ) {
        for connection in self
            .connections
            .iter()
            .filter(|connection| presentation.connection(connection.id) == State::Running)
        {
            let (origin, curve) = connection.geometry(bounds.origin + offset, zoom, preview);
            if !visible(curve, origin, bounds) {
                continue;
            }
            for marker in 0..3 {
                let t = (progress + marker as f32 / 3.) % 1.;
                let position = origin + curve.position(t);
                window.paint_quad(
                    fill(
                        Bounds::new(position - point(px(3.), px(3.)), size(px(6.), px(6.))),
                        rgb(connection.output.color),
                    )
                    .corner_radii(px(3.)),
                );
            }
        }
    }

    fn paint_grid(
        &mut self,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        zoom: f32,
        window: &mut Window,
    ) {
        let spacing = px((32. * zoom).max(12.));
        let phase = point(offset.x % spacing, offset.y % spacing);
        let key = ShapeKey::new(())
            .point(point(bounds.size.width, bounds.size.height))
            .point(phase)
            .f32(f32::from(spacing))
            .finish();
        if let Some(path) = self.grid.get(key, bounds.origin, || {
            let mut path = PathBuilder::stroke(px(0.5));
            let mut x = phase.x;
            while x < bounds.size.width {
                path.move_to(point(x, px(0.)));
                path.line_to(point(x, bounds.size.height));
                x += spacing;
            }
            let mut y = phase.y;
            while y < bounds.size.height {
                path.move_to(point(px(0.), y));
                path.line_to(point(bounds.size.width, y));
                y += spacing;
            }
            path.build().ok()
        }) {
            window.paint_path(path, gpui::rgba((theme::BORDER_STRONG << 8) | 0x66));
        }
    }
}

fn visible(curve: Curve, origin: Point<Pixels>, bounds: Bounds<Pixels>) -> bool {
    let mut curve_bounds = curve.bounds().dilate(px(8.));
    curve_bounds.origin += origin;
    curve_bounds.intersects(&bounds)
}

fn paint_connection(
    cache: &mut PathCache,
    origin: Point<Pixels>,
    curve: Curve,
    (width, state): (Pixels, State),
    color: gpui::Rgba,
    window: &mut Window,
) {
    let key = ShapeKey::new((curve.from_input, state))
        .point(curve.delta)
        .f32(width.into())
        .finish();
    if let Some(path) = cache.get(key, origin, || curve.build(width, state.dashes())) {
        window.paint_path(path, color);
    }
}
