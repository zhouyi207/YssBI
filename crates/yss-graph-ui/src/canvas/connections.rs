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
use yss_graph_document::{ConnectionId, NodeId, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;
use yss_node_protocol::PortDirection;

use super::geometry::NodeMovement;
use super::presentation::{Presentation, State};
use super::{geometry, ports};
use curve::Curve;
use yss_ui::appearance as theme;

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
    path: PathCache,
    highlight: PathCache,
    hit: hit::HitPath,
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
    nodes: Vec<geometry::NodeGeometry>,
    node_indices: BTreeMap<NodeId, usize>,
    anchors: BTreeMap<PortAddress, PortAnchor>,
    connections: Vec<Connection>,
    pending: PathCache,
    grid: PathCache,
}

impl ConnectionLayer {
    pub fn new(projection: &EditorProjectionModel) -> Self {
        let mut layer = Self {
            nodes: Vec::new(),
            node_indices: BTreeMap::new(),
            anchors: BTreeMap::new(),
            connections: Vec::new(),
            pending: PathCache::default(),
            grid: PathCache::default(),
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
                        color: ports::type_color(port),
                    });
                    anchor.node_position =
                        point(px(node.position.x as f32), px(node.position.y as f32));
                    anchor.offset = offset;
                    anchor.direction = port.direction;
                    anchor.color = ports::type_color(port);
                    self.anchors.insert(port.address.clone(), anchor);
                }
            }
            self.node_indices.insert(node.node_id, self.nodes.len());
            self.nodes.push(geometry);
        }
        let mut previous = std::mem::take(&mut self.connections)
            .into_iter()
            .map(|connection| (connection.id, connection))
            .collect::<BTreeMap<_, _>>();
        self.connections = projection
            .connections
            .iter()
            .filter_map(|connection| {
                let output = self.anchors.get(&connection.output)?.clone();
                let input = self.anchors.get(&connection.input)?.clone();
                Some(match previous.remove(&connection.connection_id) {
                    Some(mut retained) => {
                        retained.output = output;
                        retained.input = input;
                        retained
                    }
                    None => Connection {
                        id: connection.connection_id,
                        output,
                        input,
                        path: PathCache::default(),
                        highlight: PathCache::default(),
                        hit: hit::HitPath::default(),
                    },
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
        &mut self,
        bounds: Bounds<Pixels>,
        (offset, zoom): (Point<Pixels>, f32),
        preview: &NodeMovement,
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
        preview: &NodeMovement,
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
        preview: &NodeMovement,
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
        preview: &NodeMovement,
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
        let spacing = grid_spacing(zoom);
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

const GRID_WORLD_SPACING: f32 = 40.;
const GRID_MIN_SCREEN_SPACING: f32 = 20.;

fn grid_spacing(zoom: f32) -> Pixels {
    let mut spacing = GRID_WORLD_SPACING * zoom;
    // Coarser world-aligned levels keep dots legible and bound low-zoom mesh work.
    while spacing < GRID_MIN_SCREEN_SPACING {
        spacing *= 2.;
    }
    px(spacing)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_zoom_grid_keeps_world_alignment_and_bounded_screen_density() {
        let threshold = GRID_MIN_SCREEN_SPACING / GRID_WORLD_SPACING;
        let at_threshold = grid_spacing(threshold);
        assert_eq!(at_threshold, px(GRID_MIN_SCREEN_SPACING));
        assert_eq!(grid_spacing(threshold / 2.), at_threshold);

        let zoom = threshold / 5.;
        let spacing = f32::from(grid_spacing(zoom));
        assert!(spacing >= GRID_MIN_SCREEN_SPACING);
        assert!(spacing < 2. * GRID_MIN_SCREEN_SPACING);
        let world_steps = spacing / (GRID_WORLD_SPACING * zoom);
        assert_eq!(world_steps.fract(), 0.);
        assert!((world_steps as u32).is_power_of_two());

        let offset = -113_f32;
        let phase = offset.rem_euclid(spacing);
        let world_grid_index = (phase - offset) / zoom / GRID_WORLD_SPACING;
        assert_eq!(world_grid_index.fract(), 0.);
    }
}
