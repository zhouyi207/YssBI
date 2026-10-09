use std::collections::BTreeMap;

use gpui::{Bounds, Path, PathBuilder, Pixels, Point, Window, point, px, rgb};
use gpui_base::plot::{PathCache, ShapeKey};
use yss_graph_document::{ConnectionId, NodeId, NodePosition, PortAddress};
use yss_graph_editor::projection::EditorProjectionModel;

use super::geometry;
use crate::appearance;

#[derive(Clone, Copy)]
struct PortAnchor {
    node_id: NodeId,
    node_position: Point<Pixels>,
    offset: Point<Pixels>,
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
                geometry::port_offsets(node).map(|(address, offset)| {
                    (
                        address,
                        PortAnchor {
                            node_id: node.node_id,
                            node_position: point(
                                px(node.position.x as f32),
                                px(node.position.y as f32),
                            ),
                            offset,
                        },
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let connections = projection
            .connections
            .iter()
            .filter_map(|connection| {
                Some(Connection {
                    id: connection.connection_id,
                    output: *anchors.get(&connection.output)?,
                    input: *anchors.get(&connection.input)?,
                    path: PathCache::default(),
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

    pub fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        zoom: f32,
        preview: &BTreeMap<NodeId, NodePosition>,
        selected: Option<ConnectionId>,
        window: &mut Window,
    ) {
        self.paint_grid(bounds, offset, zoom, window);
        for connection in &mut self.connections {
            let output = connection.output.position(preview) * zoom;
            let delta = connection.input.position(preview) * zoom - output;
            let a = bounds.origin + offset + output;
            let b = a + delta;
            let bend = connection_bend(delta);
            // Both endpoints can be outside while the curve crosses the viewport.
            let curve_bounds = Bounds::from_corners(
                point(a.x.min(b.x - bend), a.y.min(b.y)),
                point((a.x + bend).max(b.x), a.y.max(b.y)),
            )
            .dilate(px(2.));
            if curve_bounds.intersects(&bounds) {
                paint_connection(
                    &mut connection.path,
                    a,
                    delta,
                    rgb(if selected == Some(connection.id) {
                        appearance::GREEN
                    } else {
                        appearance::BLUE
                    }),
                    window,
                );
            }
        }
    }

    pub fn paint_pending(&mut self, a: Point<Pixels>, b: Point<Pixels>, window: &mut Window) {
        paint_connection(&mut self.pending, a, b - a, rgb(appearance::AMBER), window);
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
            window.paint_path(path, gpui::rgba((appearance::BORDER_STRONG << 8) | 0x66));
        }
    }
}

fn connection_bend(delta: Point<Pixels>) -> Pixels {
    (delta.x.abs() * 0.45).max(px(45.))
}

fn build_connection(delta: Point<Pixels>) -> Option<Path<Pixels>> {
    let bend = connection_bend(delta);
    let mut path = PathBuilder::stroke(px(1.8));
    path.move_to(Point::default());
    path.cubic_bezier_to(delta, point(bend, px(0.)), delta - point(bend, px(0.)));
    path.build().ok()
}

fn paint_connection(
    cache: &mut PathCache,
    origin: Point<Pixels>,
    delta: Point<Pixels>,
    color: gpui::Rgba,
    window: &mut Window,
) {
    let key = ShapeKey::new(()).point(delta).finish();
    if let Some(path) = cache.get(key, origin, || build_connection(delta)) {
        window.paint_path(path, color);
    }
}
