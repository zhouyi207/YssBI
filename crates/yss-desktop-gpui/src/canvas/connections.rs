mod curve;
mod drag;
mod hit;
mod interaction;
mod menu;
mod render;

pub(super) use drag::ConnectionDrag;
pub(super) use interaction::ConnectionClick;

use gpui_kit::base::plot::{PathCache, ShapeKey};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, Bounds, Hsla, Pixels, Point, Window, fill, point, px, size};
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
        (offset, zoom): (Point<Pixels>, f32),
        preview: &BTreeMap<NodeId, NodePosition>,
        interaction: Interaction<'_>,
        window: &mut Window,
        cx: &App,
    ) {
        self.paint_grid(bounds, offset, zoom, window, cx);
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
                    (16., cx.theme().primary, 0.55)
                } else if hovered {
                    (7., cx.theme().primary, 0.35)
                } else {
                    (6., theme::data_color(connection.output.color, cx), 0.16)
                };
                paint_connection(
                    &mut connection.highlight,
                    origin,
                    curve,
                    (px(width), State::Valid),
                    color.opacity(alpha * opacity),
                    window,
                );
            }
            let color = if replaced {
                cx.theme().warning
            } else if state == State::Error {
                cx.theme().danger
            } else {
                theme::data_color(connection.output.color, cx)
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
                color.opacity(state.opacity() * opacity),
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
        color: Hsla,
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
            color,
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
        (offset, zoom): (Point<Pixels>, f32),
        preview: &BTreeMap<NodeId, NodePosition>,
        (presentation, progress): (&Presentation, f32),
        window: &mut Window,
        cx: &App,
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
                        theme::data_color(connection.output.color, cx),
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
        cx: &App,
    ) {
        let spacing = px(40. * zoom);
        let phase = point(
            px(f32::from(offset.x).rem_euclid(spacing.into())),
            px(f32::from(offset.y).rem_euclid(spacing.into())),
        );
        // Repeat a bounded mesh instead of tessellating the whole viewport or
        // issuing one scene primitive per dot at low zoom.
        const CELLS: usize = 32;
        let key = ShapeKey::new(()).f32(spacing.into()).finish();
        let mut y = phase.y - spacing;
        while y < bounds.size.height {
            let mut x = phase.x - spacing;
            while x < bounds.size.width {
                if let Some(path) = self.grid.get(key, bounds.origin + point(x, y), || {
                    let mut path = gpui_kit::PathBuilder::fill();
                    for row in 0..CELLS {
                        for column in 0..CELLS {
                            let x = spacing * column as f32;
                            let y = spacing * row as f32 + px(1.);
                            path.move_to(point(x, y));
                            path.arc_to(
                                point(px(1.), px(1.)),
                                px(0.),
                                false,
                                true,
                                point(x + px(2.), y),
                            );
                            path.arc_to(point(px(1.), px(1.)), px(0.), false, true, point(x, y));
                            path.close();
                        }
                    }
                    path.build().ok()
                }) {
                    window.paint_path(path, cx.theme().input);
                }
                x += spacing * CELLS as f32;
            }
            y += spacing * CELLS as f32;
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
    color: Hsla,
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
