use super::{
    axes::AxisDomain,
    frame::{self, Axis, Frame},
};
use gpui_kit::component::{
    ActiveTheme,
    plot::{IntoPlot, PathCaches, Plot, ShapeKey, TooltipState, tooltip::Tooltip},
};
use gpui_kit::{
    AnyElement, App, Bounds, ElementId, IntoElement, PathBuilder, Pixels, Point, SharedString,
    Window, point, px, size,
};
use std::sync::Arc;
use yss_application::graph::results::plot::DistributionGroup;

pub(crate) struct DistributionData {
    groups: Vec<DistributionGroup>,
    labels: Vec<String>,
    domain: AxisDomain,
    outlines: Vec<ViolinOutline>,
    pub violin: bool,
}
struct ViolinOutline {
    points: Vec<Point<Pixels>>,
    key: u64,
}
impl DistributionData {
    pub fn new(groups: Vec<DistributionGroup>, violin: bool) -> Self {
        let domain = AxisDomain::from_values(groups.iter().flat_map(|g| {
            [g.lower_whisker, g.upper_whisker]
                .into_iter()
                .chain(g.outliers.iter().copied())
                .chain(g.density.iter().filter(move |_| violin).map(|p| p.x))
        }));
        let outlines = groups
            .iter()
            .map(|g| {
                let max = g.density.iter().map(|p| p.y).fold(0_f64, f64::max);
                let width = |v| if max > 0. { (v / max) as f32 * 0.5 } else { 0. };
                let points: Vec<_> = g
                    .density
                    .iter()
                    .map(|p| point(px(-width(p.y)), px(1. - domain.position(p.x))))
                    .chain(
                        g.density
                            .iter()
                            .rev()
                            .map(|p| point(px(width(p.y)), px(1. - domain.position(p.x)))),
                    )
                    .collect();
                let mut key = ShapeKey::new("violin");
                for point in &points {
                    key.point(*point);
                }
                ViolinOutline {
                    points,
                    key: key.finish(),
                }
            })
            .collect();
        Self {
            labels: groups.iter().map(|g| g.label.clone()).collect(),
            groups,
            domain,
            outlines,
            violin,
        }
    }
    fn frame(&self, bounds: Bounds<Pixels>) -> Option<Frame<'_>> {
        Frame::new(
            bounds,
            70.,
            16.,
            36.,
            Axis::Labels(&self.labels),
            Axis::Numeric(self.domain),
        )
    }
}
#[derive(IntoPlot)]
pub(crate) struct Distribution {
    pub data: Arc<DistributionData>,
    pub id: SharedString,
}
impl Plot for Distribution {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(frame) = self.data.frame(bounds) else {
            return;
        };
        frame.axes(window, cx);
        let palette = [
            cx.theme().chart_1,
            cx.theme().chart_2,
            cx.theme().chart_3,
            cx.theme().chart_4,
            cx.theme().chart_5,
        ];
        let band = frame.bounds.size.width / self.data.groups.len() as f32 * 0.7;
        let caches = PathCaches::for_paint(self.id.clone(), window, cx);
        caches.update(cx, |caches, _| {
            window.with_content_mask(
                Some(gpui_kit::ContentMask {
                    bounds: frame.bounds,
                }),
                |window| {
                    for (i, g) in self.data.groups.iter().enumerate() {
                        let color = palette[i % palette.len()];
                        let center = frame.point(i as f64, 0.).x;
                        let y = |value| frame.point(0., value).y;
                        let width = band * if self.data.violin { 0.18 } else { 0.7 };
                        if self.data.violin {
                            let outline = &self.data.outlines[i];
                            let mut key = ShapeKey::new(outline.key);
                            key.f32(band.as_f32())
                                .f32(frame.bounds.size.height.as_f32());
                            let project = |p: &Point<Pixels>| {
                                point(p.x * band.as_f32(), p.y * frame.bounds.size.height.as_f32())
                            };
                            let (fill_cache, stroke_cache) = caches.slot_pair(i);
                            for (cache, stroke) in [(fill_cache, false), (stroke_cache, true)] {
                                if let Some(path) = cache.get(
                                    key.finish(),
                                    point(center, frame.bounds.origin.y),
                                    || {
                                        let mut builder = if stroke {
                                            PathBuilder::stroke(px(1.))
                                        } else {
                                            PathBuilder::fill()
                                        };
                                        let first = outline.points.first()?;
                                        builder.move_to(project(first));
                                        for p in outline.points.iter().skip(1) {
                                            builder.line_to(project(p));
                                        }
                                        builder.close();
                                        builder.build().ok()
                                    },
                                ) {
                                    window.paint_path(
                                        path,
                                        if stroke { color } else { color.opacity(0.25) },
                                    );
                                }
                            }
                        }
                        frame::line(
                            point(center, y(g.lower_whisker)),
                            point(center, y(g.upper_whisker)),
                            1.,
                            color,
                            window,
                        );
                        let left = center - width / 2.;
                        let right = center + width / 2.;
                        frame::rect(
                            Bounds::new(
                                point(left, y(g.q3)),
                                size(width, (y(g.q1) - y(g.q3)).max(px(1.))),
                            ),
                            color.opacity(if self.data.violin { 0.6 } else { 0.25 }),
                            window,
                        );
                        for x in [left, right] {
                            frame::line(point(x, y(g.q3)), point(x, y(g.q1)), 1., color, window);
                        }
                        for value in [g.lower_whisker, g.q1, g.median, g.q3, g.upper_whisker] {
                            frame::line(
                                point(left, y(value)),
                                point(right, y(value)),
                                if value == g.median { 2. } else { 1. },
                                color,
                                window,
                            );
                        }
                        if !self.data.violin {
                            for value in &g.outliers {
                                frame::dot(point(center, y(*value)), 2.5, color, window);
                            }
                        }
                    }
                },
            );
        });
    }
    fn tooltip_state(
        &self,
        p: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        let frame = self
            .data
            .frame(Bounds::new(point(px(0.), px(0.)), bounds.size))?;
        if !frame.bounds.contains(&p) {
            return None;
        }
        let index = (((p.x - frame.bounds.origin.x) / frame.bounds.size.width)
            * self.data.groups.len() as f32) as usize;
        self.data.groups.get(index)?;
        Some(TooltipState::new(index, p, vec![]))
    }
    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let g = self.data.groups.get(state.index)?;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(g.label.clone())
                .plain_row("N", g.observations.to_string())
                .plain_row("Q1", frame::number(g.q1))
                .plain_row(
                    crate::text::translate("plot.median"),
                    frame::number(g.median),
                )
                .plain_row("Q3", frame::number(g.q3))
                .plain_row(
                    crate::text::translate("native.plots.whiskers"),
                    format!(
                        "[{}, {}]",
                        frame::number(g.lower_whisker),
                        frame::number(g.upper_whisker)
                    ),
                )
                .plain_row(
                    crate::text::translate("plot.outliers"),
                    g.outlier_count.to_string(),
                )
                .into_any_element(),
        )
    }
}
