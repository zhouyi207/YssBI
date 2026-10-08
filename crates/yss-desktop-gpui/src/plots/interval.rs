use super::{
    axes::AxisDomain,
    frame::{self, Axis, Frame},
};
use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString, Window, point, px,
};
use gpui_component::{
    ActiveTheme,
    plot::{IntoPlot, Plot, TooltipState, tooltip::Tooltip},
};
use std::{ops::Range, sync::Arc};
use yss_application::graph::results::plot::{CoefficientPoint, IntervalPoint};
const PAGE_SIZE: usize = 100;

pub(crate) struct IntervalData {
    rows: Vec<IntervalPoint>,
    labels: Vec<String>,
    x: Vec<AxisDomain>,
    y: AxisDomain,
    horizontal: bool,
}
impl IntervalData {
    pub fn errors(rows: Vec<IntervalPoint>) -> Self {
        Self {
            x: vec![AxisDomain::from_values(rows.iter().map(|p| p.x))],
            y: AxisDomain::from_values(rows.iter().flat_map(|p| [p.lower, p.upper])),
            labels: vec![],
            rows,
            horizontal: false,
        }
    }
    pub fn coefficients(data: Vec<CoefficientPoint>) -> Self {
        let x = data
            .chunks(PAGE_SIZE)
            .map(|chunk| {
                AxisDomain::from_values(
                    std::iter::once(0.).chain(chunk.iter().flat_map(|p| [p.lower, p.upper])),
                )
            })
            .collect();
        let labels = data.iter().map(|p| p.label.clone()).collect();
        let rows = data
            .into_iter()
            .map(|p| IntervalPoint {
                x: p.value,
                y: 0.,
                lower: p.lower,
                upper: p.upper,
            })
            .collect();
        Self {
            rows,
            labels,
            x,
            y: AxisDomain::fixed([0., 1.]),
            horizontal: true,
        }
    }
    pub fn page_count(&self) -> usize {
        self.x.len()
    }
    pub fn count(&self) -> usize {
        self.rows.len()
    }
    fn range(&self, page: usize) -> Range<usize> {
        if self.horizontal {
            let start = page * PAGE_SIZE;
            start..(start + PAGE_SIZE).min(self.rows.len())
        } else {
            0..self.rows.len()
        }
    }
    fn frame(&self, bounds: Bounds<Pixels>, page: usize) -> Option<Frame<'_>> {
        let y = if self.horizontal {
            Axis::Labels(&self.labels[self.range(page)])
        } else {
            Axis::Numeric(self.y)
        };
        Frame::new(
            bounds,
            if self.horizontal { 112. } else { 70. },
            16.,
            36.,
            Axis::Numeric(self.x[page]),
            y,
        )
    }
    fn position(&self, frame: &Frame<'_>, i: usize, p: &IntervalPoint) -> Point<Pixels> {
        frame.point(p.x, if self.horizontal { i as f64 } else { p.y })
    }
}
#[derive(IntoPlot)]
pub(crate) struct Interval {
    pub data: Arc<IntervalData>,
    pub id: SharedString,
    pub page: usize,
}
impl Plot for Interval {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(frame) = self.data.frame(bounds, self.page) else {
            return;
        };
        frame.axes(window, cx);
        let color = cx.theme().primary;
        window.with_content_mask(
            Some(gpui::ContentMask {
                bounds: frame.bounds,
            }),
            |window| {
                if self.data.horizontal {
                    let x = frame.point(0., 0.).x;
                    frame::line(
                        point(x, frame.bounds.origin.y),
                        point(x, frame.bounds.bottom()),
                        1.,
                        cx.theme().muted_foreground,
                        window,
                    );
                }
                for (i, p) in self.data.rows[self.data.range(self.page)]
                    .iter()
                    .enumerate()
                {
                    let center = self.data.position(&frame, i, p);
                    let (a, b) = if self.data.horizontal {
                        (
                            point(frame.point(p.lower, 0.).x, center.y),
                            point(frame.point(p.upper, 0.).x, center.y),
                        )
                    } else {
                        (
                            point(center.x, frame.point(0., p.lower).y),
                            point(center.x, frame.point(0., p.upper).y),
                        )
                    };
                    frame::line(a, b, 1.5, color, window);
                    let cap = if self.data.horizontal {
                        point(px(0.), px(5.))
                    } else {
                        point(px(5.), px(0.))
                    };
                    for end in [a, b] {
                        frame::line(end - cap, end + cap, 1., color, window);
                    }
                    frame::dot(center, 3.5, color, window);
                }
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
        let (index, target, distance) = self.data.rows[range.clone()]
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let target = self.data.position(&frame, i, row);
                let distance = if self.data.horizontal {
                    (target.y - p.y).as_f32().abs()
                } else {
                    (target.x - p.x).as_f32().hypot((target.y - p.y).as_f32())
                };
                (i, target, distance)
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))?;
        (distance
            <= if self.data.horizontal {
                (frame.bounds.size.height.as_f32() / range.len() as f32 / 2.).max(2.)
            } else {
                12.
            })
        .then_some(TooltipState::new(range.start + index, target, vec![target]))
    }
    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let p = self.data.rows.get(state.index)?;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(if self.data.horizontal {
                    self.data.labels[state.index].clone()
                } else {
                    frame::number(p.x)
                })
                .plain_row(
                    crate::text::translate("native.plots.estimate"),
                    frame::number(if self.data.horizontal { p.x } else { p.y }),
                )
                .plain_row(
                    crate::text::translate("native.plots.lower"),
                    frame::number(p.lower),
                )
                .plain_row(
                    crate::text::translate("native.plots.upper"),
                    frame::number(p.upper),
                )
                .into_any_element(),
        )
    }
}
