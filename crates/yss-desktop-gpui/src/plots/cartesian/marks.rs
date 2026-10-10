use super::{CartesianKind, CartesianPlot};
use gpui_kit::component::{
    ActiveTheme,
    plot::{
        Curve, PathCaches,
        shape::{Area, Line},
    },
};
use gpui_kit::{App, Bounds, Pixels, Window, fill, point, px, size};
use yss_application::chart::PlotPoint;

impl CartesianPlot {
    pub(super) fn paint_marks(&self, chart: &Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let color = cx.theme().primary;
        let reference_color = cx.theme().muted_foreground;
        let (x, y) = (self.data.x, self.data.y);
        let (width, height) = (chart.size.width.as_f32(), chart.size.height.as_f32());
        if self.data.zero_line {
            let baseline = self.data.position(&PlotPoint { x: 0., y: 0. }, chart).y;
            super::super::frame::line(
                point(chart.origin.x, baseline),
                point(chart.right(), baseline),
                1.,
                reference_color,
                window,
            );
        }
        let caches = PathCaches::for_paint(self.id.clone(), window, cx);
        caches.update(cx, |caches, _| {
            for (index, reference) in self.data.reference_lines.iter().enumerate() {
                Line::new()
                    .data(reference.iter().copied())
                    .x(move |p: &PlotPoint| Some(x.position(p.x) * width))
                    .y(move |p: &PlotPoint| Some((1. - y.position(p.y)) * height))
                    .curve(Curve::Linear)
                    .stroke(reference_color.opacity(0.65))
                    .stroke_width(px(1.))
                    .paint_cached(chart, caches.slot(index + 3), window);
            }
            if self.data.kind == CartesianKind::Density {
                let (fill_cache, stroke_cache) = caches.slot_pair(1);
                Area::new()
                    .data(self.data.result.data.iter().copied())
                    .x(move |p: &PlotPoint| Some(x.position(p.x) * width))
                    .y0((1. - y.position(0.)) * height)
                    .y1(move |p: &PlotPoint| Some((1. - y.position(p.y)) * height))
                    .curve(Curve::Linear)
                    .fill(color.opacity(0.2))
                    .paint_cached(chart, fill_cache, stroke_cache, window);
            }
            if self.data.kind != CartesianKind::Scatter {
                let first = (self.data.kind == CartesianKind::Ecdf)
                    .then_some(PlotPoint { x: x.at(0.), y: 0. });
                // Component StepAfter omits its final vertical segment. Repeating the
                // endpoint closes that jump without changing the empirical observations.
                let last = (self.data.kind == CartesianKind::Ecdf)
                    .then(|| self.data.result.data.last().copied())
                    .flatten();
                let mut line = Line::new()
                    .data(
                        first
                            .into_iter()
                            .chain(self.data.result.data.iter().copied())
                            .chain(last),
                    )
                    .x(move |p: &PlotPoint| Some(x.position(p.x) * width))
                    .y(move |p: &PlotPoint| Some((1. - y.position(p.y)) * height))
                    .curve(if self.data.kind == CartesianKind::Ecdf {
                        Curve::StepAfter
                    } else {
                        Curve::Linear
                    })
                    .stroke(color)
                    .stroke_width(px(2.));
                if self.show_points {
                    line = line.dot().dot_size(px(4.)).dot_fill(color.opacity(0.7));
                }
                line.paint_cached(chart, caches.slot(0), window);
            }
        });
        if self.data.kind == CartesianKind::Scatter {
            // Draw highlighted observations last so overlaps cannot hide them.
            for highlighted in [false, true] {
                if highlighted && self.data.observations.is_none() {
                    break;
                }
                for (index, datum) in self.data.result.data.iter().enumerate() {
                    if self.data.highlighted(index) != highlighted {
                        continue;
                    }
                    let radius = px(self.data.radius(index));
                    if radius <= px(0.) {
                        continue;
                    }
                    let position = self.data.position(datum, chart);
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                position - point(radius, radius),
                                size(radius * 2., radius * 2.),
                            ),
                            if highlighted {
                                cx.theme().warning
                            } else {
                                color.opacity(0.7)
                            },
                        )
                        .corner_radii(radius),
                    );
                }
            }
        }
    }
}
