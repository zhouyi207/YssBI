//! What the canvas paints each frame, back to front: grid, connections, nodes, the wire
//! being dragged and the marquee. Run activity animates in its own view, so a running
//! graph repaints only its pulses, not every node.
use std::time::Instant;

use gpui_kit::{
    App, BorderStyle, Bounds, Context, IntoElement, Pixels, Point, Render, WeakEntity, Window,
    canvas, fill, point, prelude::*, px, quad, size,
};

use super::{GraphCanvas, connections, presentation::State, style::CanvasStyle};

/// The finest grid spacing in world units; coarser levels double it.
const GRID_SPACING: f32 = 24.;
/// Screen spacing below which a grid level fades out entirely.
const GRID_FADE_START: f32 = 14.;
/// Screen spacing at and above which a grid level is fully shown.
const GRID_FADE_END: f32 = 28.;
/// Screen pixels a running pulse travels per second.
const PULSE_SPEED: f32 = 42.;

impl GraphCanvas {
    pub(super) fn paint_scene(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &App) {
        let style = CanvasStyle::new(cx);
        paint_grid(bounds, self.offset, self.zoom, &style, window);
        let drag = self.connection_drag();
        self.connection_layer.paint(
            bounds,
            (self.offset, self.zoom),
            &self.preview,
            connections::Interaction {
                presentation: &self.presentation,
                selected: &self.selected_connections,
                hovered: self.hovered_connection(),
                replacements: drag.map(|drag| &drag.replaced),
            },
            &style,
            window,
        );
        self.paint_nodes(bounds, &style, window, cx);
        if let Some(pending) = self.pending_connection(cx) {
            self.connection_layer.paint_pending(
                (bounds.origin + pending.start, bounds.origin + pending.end),
                (pending.from_input, self.zoom),
                pending.color,
                window,
            );
        }
        if let Some(marquee) = self.selection_bounds() {
            window.paint_quad(quad(
                marquee + bounds.origin,
                px(2.),
                style.selection_fill,
                px(1.),
                style.selection,
                BorderStyle::Solid,
            ));
        }
    }

    /// Whether anything visible is running, which keeps the activity view animating.
    pub(super) fn activity_visible(&self) -> bool {
        let bounds = self.bounds.get();
        let running_nodes = self
            .presentation
            .nodes
            .values()
            .any(|display| display.state == State::Running);
        (running_nodes
            && self.painted_nodes().any(|(node, geometry)| {
                self.presentation
                    .nodes
                    .get(&node.node_id)
                    .is_some_and(|display| display.state == State::Running)
                    && self
                        .node_screen_rect(node, geometry, bounds.origin)
                        .intersects(&bounds)
            }))
            || self.connection_layer.has_visible_running(
                &self.presentation,
                bounds,
                self.offset,
                self.zoom,
                &self.preview,
            )
    }

    fn paint_activity(&self, bounds: Bounds<Pixels>, seconds: f32, window: &mut Window, cx: &App) {
        let style = CanvasStyle::new(cx);
        let fade = if self.connection_drag().is_some() {
            0.25
        } else {
            1.
        };
        let mut occluders = Vec::new();
        let mut spinners = Vec::new();
        for (node, geometry) in self.painted_nodes() {
            let rect = self.node_screen_rect(node, geometry, bounds.origin);
            if !rect.intersects(&bounds) {
                continue;
            }
            occluders.push(rect);
            let running = self
                .presentation
                .nodes
                .get(&node.node_id)
                .is_some_and(|display| display.state == State::Running);
            if running && !geometry.layout.compact {
                spinners.push(self.status_center(rect));
            }
        }
        self.connection_layer.paint_activity(
            bounds,
            (self.offset, self.zoom),
            &self.preview,
            (&self.presentation, seconds * PULSE_SPEED),
            &occluders,
            (&style, fade),
            window,
        );
        let radius = px(super::nodes::ICON_SIZE * self.zoom * 0.45);
        if radius < px(2.) {
            return;
        }
        let turn = seconds * std::f32::consts::TAU * 0.8;
        let mut tessellator = connections::Tessellator::default();
        let mut path = None;
        for center in spinners {
            tessellator.arc(
                center,
                radius,
                (turn, std::f32::consts::TAU * 0.7),
                (radius * 0.3).max(px(1.25)),
                &mut path,
            );
        }
        if let Some(path) = path {
            window.paint_path(path, style.info.opacity(fade));
        }
    }
}

/// Dots on a world-aligned lattice. The finer of two adjacent levels fades in as zoom
/// spreads it out, so density stays bounded and zooming never pops.
fn paint_grid(
    bounds: Bounds<Pixels>,
    offset: Point<Pixels>,
    zoom: f32,
    style: &CanvasStyle,
    window: &mut Window,
) {
    let (spacing, fine) = grid_level(zoom);
    let dot = px(if zoom < 0.6 { 1.5 } else { 2. });
    let first = |offset: f32| (-offset / spacing).ceil() as i64;
    let (first_x, first_y) = (first(offset.x.into()), first(offset.y.into()));
    let columns = (f32::from(bounds.size.width) / spacing).ceil() as i64 + 1;
    let rows = (f32::from(bounds.size.height) / spacing).ceil() as i64 + 1;
    window.paint_layer(bounds, |window| {
        for row in first_y..first_y + rows {
            for column in first_x..first_x + columns {
                // Every other dot of this level belongs to the next, always-visible one.
                let coarse = row % 2 == 0 && column % 2 == 0;
                let alpha = if coarse { 1. } else { fine * fine };
                if alpha < 0.04 {
                    continue;
                }
                let center = bounds.origin
                    + offset
                    + point(px(column as f32 * spacing), px(row as f32 * spacing));
                window.paint_quad(
                    fill(
                        Bounds::new(center - point(dot / 2., dot / 2.), size(dot, dot)),
                        style.grid.opacity(alpha),
                    )
                    .corner_radii(dot / 2.),
                );
            }
        }
    });
}

/// The screen spacing of the finest grid level drawn at `zoom`, and how far it has faded in.
fn grid_level(zoom: f32) -> (f32, f32) {
    let mut spacing = GRID_SPACING * zoom;
    // Coarser world-aligned levels keep dots legible and bound low-zoom work.
    while spacing < GRID_FADE_START {
        spacing *= 2.;
    }
    let fine = ((spacing - GRID_FADE_START) / (GRID_FADE_END - GRID_FADE_START)).clamp(0., 1.);
    (spacing, fine)
}

pub(super) struct ActivityOverlay {
    canvas: WeakEntity<GraphCanvas>,
    epoch: Instant,
}

impl ActivityOverlay {
    pub fn new(canvas: WeakEntity<GraphCanvas>) -> Self {
        Self {
            canvas,
            epoch: Instant::now(),
        }
    }
}

impl Render for ActivityOverlay {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let owner = self.canvas.clone();
        let epoch = self.epoch;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                let Some(view) = owner.upgrade() else {
                    return;
                };
                let still = cx.reduce_motion();
                let seconds = if still {
                    0.
                } else {
                    epoch.elapsed().as_secs_f32()
                };
                view.read(cx).paint_activity(bounds, seconds, window, cx);
                if !still {
                    window.request_animation_frame();
                }
            },
        )
        .absolute()
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_levels_stay_world_aligned_with_bounded_screen_density() {
        for zoom in [0.1, 0.13, 0.25, 0.4, 0.58, 0.6, 1., 1.7, 5.] {
            let (spacing, fine) = grid_level(zoom);
            assert!(spacing >= GRID_FADE_START, "zoom {zoom}: {spacing}");
            assert!(spacing < 2. * GRID_FADE_START.max(GRID_SPACING * zoom) + f32::EPSILON);
            let steps = spacing / (GRID_SPACING * zoom);
            assert!((steps.round() - steps).abs() < 1e-4);
            assert!((steps.round() as u32).is_power_of_two());
            assert!((0. ..=1.).contains(&fine));
        }
        // The finer level fades in continuously instead of appearing at once.
        let (_, start) = grid_level(GRID_FADE_START / GRID_SPACING);
        let (_, middle) = grid_level((GRID_FADE_START + GRID_FADE_END) / 2. / GRID_SPACING);
        let (_, end) = grid_level(GRID_FADE_END / GRID_SPACING);
        assert_eq!(start, 0.);
        assert!(middle > 0. && middle < 1.);
        assert_eq!(end, 1.);
    }
}
