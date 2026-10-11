//! Type glyphs use native paths; cached tessellation is independent of hover and animation.
use std::time::Duration;

use gpui_kit::base::plot::{PathCaches, ShapeKey};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    Animation, AnimationExt, AnyElement, App, Bounds, Hsla, IntoElement, Path, PathBuilder, Pixels,
    canvas, div, fill, point, prelude::*, px, size,
};
use yss_data_contract::ValueType;
use yss_graph_editor::projection::EditorPortTypeState;

#[derive(Clone, Copy, Debug, Hash)]
enum Shape {
    Circle,
    Array,
    Series,
    Frame,
    Struct,
}

#[derive(Clone, Copy)]
pub(super) struct Glyph {
    shape: Shape,
    dashed: bool,
}

impl Glyph {
    pub fn new(state: &EditorPortTypeState) -> Self {
        let data_type = match state {
            EditorPortTypeState::Exact { data_type, .. } => data_type.as_ref(),
            _ => None,
        };
        Self {
            shape: match data_type {
                Some(ValueType::Array(_)) => Shape::Array,
                Some(ValueType::DataSeries(_)) => Shape::Series,
                Some(ValueType::DataFrame) => Shape::Frame,
                Some(ValueType::Struct(_)) => Shape::Struct,
                _ => Shape::Circle,
            },
            dashed: matches!(data_type, Some(ValueType::OneOf(_))),
        }
    }

    pub fn render(
        self,
        diameter: Pixels,
        color: Hsla,
        connected: bool,
        pulse: bool,
        cx: &App,
    ) -> AnyElement {
        let view = div().id("glyph").size(diameter).child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    let caches = PathCaches::for_paint("port-shape", window, cx);
                    caches.update(cx, |caches, cx| {
                        let scale = bounds.size.width / px(12.);
                        for (slot, fill) in [true, false].into_iter().enumerate() {
                            let key = ShapeKey::new((self.shape, self.dashed, fill))
                                .f32(scale)
                                .finish();
                            if let Some(path) = caches
                                .slot(slot)
                                .get(key, bounds.origin, || self.path(scale, fill))
                            {
                                let ink = if fill {
                                    color.opacity(if connected { 1. } else { 0.05 })
                                } else if connected || pulse {
                                    color
                                } else {
                                    cx.theme().muted_foreground
                                };
                                window.paint_path(path, ink);
                            }
                        }
                        if matches!(self.shape, Shape::Frame) {
                            let ink = if connected {
                                cx.theme().table
                            } else {
                                cx.theme().muted_foreground
                            };
                            let unit = px(scale);
                            for (x, y, width, height) in [(1.5, 4.1, 9., 0.8), (4.6, 1.5, 0.8, 9.)]
                            {
                                window.paint_quad(fill(
                                    Bounds::new(
                                        bounds.origin + point(unit * x, unit * y),
                                        size(unit * width, unit * height),
                                    ),
                                    ink,
                                ));
                            }
                        }
                    });
                },
            )
            .size_full(),
        );
        let view = view.when(connected, |view| {
            view.relative().child(
                div()
                    .absolute()
                    .left(diameter * 0.4)
                    .top(diameter * 0.4)
                    .size(diameter * 0.2)
                    .rounded_full()
                    .bg(cx.theme().table),
            )
        });
        if pulse {
            // The static diagnostic ring remains visible with reduced motion.
            view.with_animation(
                "unbound-input",
                Animation::new(Duration::from_millis(1200)).repeat(),
                |view, progress| {
                    view.opacity(0.65 + 0.35 * (progress * std::f32::consts::PI).sin())
                },
            )
            .into_any_element()
        } else {
            view.into_any_element()
        }
    }

    fn path(self, scale: f32, fill: bool) -> Option<Path<Pixels>> {
        let p = |x, y| point(px(x * scale), px(y * scale));
        let mut path = if fill {
            PathBuilder::fill()
        } else {
            PathBuilder::stroke(px(1.5 * scale))
        };
        if self.dashed && !fill {
            path = path.dash_array(&[px(2. * scale), px(2. * scale)]);
        }
        match self.shape {
            Shape::Circle => {
                path.move_to(p(1.5, 6.));
                path.arc_to(p(4.5, 4.5), px(0.), false, true, p(10.5, 6.));
                path.arc_to(p(4.5, 4.5), px(0.), false, true, p(1.5, 6.));
                path.close();
            }
            Shape::Array | Shape::Frame => {
                let (left, right, radius) = if matches!(self.shape, Shape::Array) {
                    (2., 10., 1.5)
                } else {
                    (1.5, 10.5, 1.)
                };
                path.move_to(p(left + radius, left));
                path.line_to(p(right - radius, left));
                path.curve_to(p(right, left + radius), p(right, left));
                path.line_to(p(right, right - radius));
                path.curve_to(p(right - radius, right), p(right, right));
                path.line_to(p(left + radius, right));
                path.curve_to(p(left, right - radius), p(left, right));
                path.line_to(p(left, left + radius));
                path.curve_to(p(left + radius, left), p(left, left));
                path.close();
            }
            Shape::Series => {
                path.add_polygon(&[p(6., 1.), p(11., 6.), p(6., 11.), p(1., 6.)], true);
            }
            Shape::Struct => {
                path.add_polygon(
                    &[
                        p(6., 1.),
                        p(10.5, 3.5),
                        p(10.5, 8.5),
                        p(6., 11.),
                        p(1.5, 8.5),
                        p(1.5, 3.5),
                    ],
                    true,
                );
            }
        }
        path.build().ok()
    }
}
