use super::{
    axes::AxisDomain,
    frame::{self, Axis, Frame},
};
use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString, Window, point,
    px, size,
};
use gpui_component::{
    ActiveTheme,
    plot::{
        Curve, IntoPlot, PathCaches, Plot, PlotLabel, TooltipState, label::Text, shape::Line,
        tooltip::Tooltip,
    },
};
use std::{ops::Range, sync::Arc};
use yss_application::graph::results::plot::{CombinationPlot, ParetoCategory};

const PAGE_SIZE: usize = 100;
enum Bars {
    Counts(Vec<usize>),
    Values(Vec<f64>),
}
impl Bars {
    fn value(&self, index: usize) -> f64 {
        match self {
            Self::Counts(values) => values[index] as f64,
            Self::Values(values) => values[index],
        }
    }
    fn label(&self, index: usize) -> String {
        match self {
            Self::Counts(values) => values[index].to_string(),
            Self::Values(values) => frame::number(values[index]),
        }
    }
}
struct Domains {
    left: AxisDomain,
    right: AxisDomain,
}
pub(crate) struct CompositeData {
    labels: Vec<String>,
    bars: Bars,
    line: Vec<f64>,
    domains: Vec<Domains>,
    dual_axis: bool,
}
impl CompositeData {
    pub fn pareto(data: Vec<ParetoCategory>) -> Self {
        let mut labels = Vec::with_capacity(data.len());
        let mut counts = Vec::with_capacity(data.len());
        let mut line = Vec::with_capacity(data.len());
        for row in data {
            labels.push(row.label);
            counts.push(row.count);
            line.push(row.cumulative);
        }
        Self::new(labels, Bars::Counts(counts), line, true)
    }
    pub fn combination(plot: CombinationPlot) -> Self {
        Self::new(
            plot.labels,
            Bars::Values(plot.bars),
            plot.line,
            plot.dual_axis,
        )
    }
    fn new(labels: Vec<String>, bars: Bars, line: Vec<f64>, dual_axis: bool) -> Self {
        let pareto = matches!(bars, Bars::Counts(_));
        let page_size = if pareto {
            PAGE_SIZE
        } else {
            labels.len().max(1)
        };
        let domains = line
            .chunks(page_size)
            .enumerate()
            .map(|(page, values)| {
                let start = page * page_size;
                let left = AxisDomain::from_values(
                    std::iter::once(0.)
                        .chain((start..start + values.len()).map(|i| bars.value(i)))
                        .chain(values.iter().copied().filter(|_| !dual_axis)),
                );
                let right = if pareto {
                    AxisDomain::fixed([0., 1.])
                } else if dual_axis {
                    AxisDomain::from_values(values.iter().copied())
                } else {
                    left
                };
                Domains { left, right }
            })
            .collect();
        Self {
            labels,
            bars,
            line,
            domains,
            dual_axis,
        }
    }
    fn pareto_kind(&self) -> bool {
        matches!(self.bars, Bars::Counts(_))
    }
    pub fn count(&self) -> usize {
        self.labels.len()
    }
    pub fn page_count(&self) -> usize {
        self.domains.len()
    }
    fn range(&self, page: usize) -> Range<usize> {
        if self.pareto_kind() {
            let start = page * PAGE_SIZE;
            start..(start + PAGE_SIZE).min(self.count())
        } else {
            0..self.count()
        }
    }
    fn frame(&self, bounds: Bounds<Pixels>, page: usize) -> Option<Frame<'_>> {
        let mut frame = Frame::new(
            bounds,
            72.,
            30.,
            38.,
            Axis::Labels(&self.labels[self.range(page)]),
            Axis::Numeric(self.domains[page].left),
        )?;
        if self.dual_axis {
            // Frame already reserves 24 px on the right; the second scale needs 64.
            frame.bounds.size.width -= px(40.);
        }
        (frame.bounds.size.width > px(24.)).then_some(frame)
    }
    fn legend_keys(&self) -> [&'static str; 2] {
        if self.pareto_kind() {
            ["plot.frequency", "plot.cumulative"]
        } else {
            ["plot.bars", "plot.line"]
        }
    }
}

#[derive(IntoPlot)]
pub(crate) struct Composite {
    pub data: Arc<CompositeData>,
    pub id: SharedString,
    pub page: usize,
}
impl Plot for Composite {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(frame) = self.data.frame(bounds, self.page) else {
            return;
        };
        frame.axes(window, cx);
        let right = self.data.domains[self.page].right;
        if self.data.dual_axis {
            frame.right_axis(right, self.data.pareto_kind(), window, cx);
        }
        let primary = cx.theme().primary;
        let secondary = cx.theme().yellow;
        PlotLabel::new(
            self.data
                .legend_keys()
                .into_iter()
                .enumerate()
                .map(|(i, key)| {
                    Text::new(
                        crate::text::translate(key),
                        point(px(i as f32 * 130.), px(-22.)),
                        if i == 0 { primary } else { secondary },
                    )
                })
                .collect(),
        )
        .paint(&frame.bounds, window, cx);
        let range = self.data.range(self.page);
        let (width, height) = (
            frame.bounds.size.width.as_f32(),
            frame.bounds.size.height.as_f32(),
        );
        let count = range.len() as f32;
        let bar_width = frame.bounds.size.width / count * 0.85;
        let baseline = frame.point(0., 0.).y;
        let caches = PathCaches::for_paint(self.id.clone(), window, cx);
        window.with_content_mask(
            Some(gpui::ContentMask {
                bounds: frame.bounds,
            }),
            |window| {
                for (i, index) in range.clone().enumerate() {
                    let target = frame.point(i as f64, self.data.bars.value(index));
                    frame::rect(
                        Bounds::new(
                            point(target.x - bar_width / 2., target.y.min(baseline)),
                            size(bar_width, (target.y - baseline).abs()),
                        ),
                        primary.opacity(0.65),
                        window,
                    );
                }
                caches.update(cx, |caches, _| {
                    Line::new()
                        .data(self.data.line[range].iter().copied().enumerate())
                        .x(move |(i, _): &(usize, f64)| Some((*i as f32 + 0.5) / count * width))
                        .y(move |(_, value): &(usize, f64)| {
                            Some((1. - right.position(*value)) * height)
                        })
                        .curve(Curve::Linear)
                        .stroke(secondary)
                        .stroke_width(px(2.))
                        .dot()
                        .dot_size(px(6.))
                        .dot_fill(secondary)
                        .paint_cached(&frame.bounds, caches.slot(0), window);
                });
            },
        );
    }
    fn tooltip_state(
        &self,
        p: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        let frame = self
            .data
            .frame(Bounds::new(point(px(0.), px(0.)), bounds.size), self.page)?;
        if !frame.bounds.contains(&p) {
            return None;
        }
        let range = self.data.range(self.page);
        let i = (((p.x - frame.bounds.origin.x) / frame.bounds.size.width) * range.len() as f32)
            as usize;
        let index = range.start + i;
        if index >= range.end {
            return None;
        }
        let target = point(
            frame.point(i as f64, 0.).x,
            frame.bounds.origin.y
                + frame.bounds.size.height
                    * (1.
                        - self.data.domains[self.page]
                            .right
                            .position(self.data.line[index])),
        );
        Some(TooltipState::new(index, target, vec![target]))
    }
    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let label = self.data.labels.get(state.index)?;
        let keys = self.data.legend_keys();
        let value = self.data.line[state.index];
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(label.clone())
                .plain_row(
                    crate::text::translate(keys[0]),
                    self.data.bars.label(state.index),
                )
                .plain_row(
                    crate::text::translate(keys[1]),
                    if self.data.pareto_kind() {
                        format!("{:.1}%", value * 100.)
                    } else {
                        frame::number(value)
                    },
                )
                .into_any_element(),
        )
    }
}
