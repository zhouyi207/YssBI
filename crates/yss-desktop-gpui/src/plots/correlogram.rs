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
    plot::{IntoPlot, Plot, TooltipState, tooltip::Tooltip},
};
use std::sync::Arc;
use yss_application::graph::results::plot::CorrelogramPoint;

pub(crate) struct CorrelogramData {
    points: Vec<CorrelogramPoint>,
    labels: Vec<String>,
    ci: f64,
    domain: AxisDomain,
}
impl CorrelogramData {
    pub fn new(points: Vec<CorrelogramPoint>, ci: f64) -> Self {
        let extent = (ci * 1.2).clamp(1., f64::MAX);
        Self {
            labels: points.iter().map(|p| p.lag.to_string()).collect(),
            points,
            ci,
            domain: AxisDomain::fixed([-extent, extent]),
        }
    }
    fn frame(&self, bounds: Bounds<Pixels>) -> Option<Frame<'_>> {
        Frame::new(
            bounds,
            60.,
            10.,
            30.,
            Axis::Labels(&self.labels),
            Axis::Numeric(self.domain),
        )
    }
}
#[derive(IntoPlot)]
pub(crate) struct Correlogram {
    pub data: Arc<CorrelogramData>,
    pub id: SharedString,
    pub partial: bool,
}
impl Plot for Correlogram {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(frame) = self.data.frame(bounds) else {
            return;
        };
        frame.axes(window, cx);
        let y = |v| frame.point(0., v).y;
        let left = frame.bounds.origin.x;
        let right = left + frame.bounds.size.width;
        frame::rect(
            Bounds::new(
                point(left, y(self.data.ci)),
                size(frame.bounds.size.width, y(-self.data.ci) - y(self.data.ci)),
            ),
            cx.theme().muted.opacity(0.6),
            window,
        );
        for value in [-self.data.ci, 0., self.data.ci] {
            frame::line(
                point(left, y(value)),
                point(right, y(value)),
                1.,
                cx.theme().muted_foreground.opacity(0.6),
                window,
            );
        }
        let width = frame.bounds.size.width / self.data.points.len() as f32 * 0.7;
        window.with_content_mask(
            Some(gpui::ContentMask {
                bounds: frame.bounds,
            }),
            |window| {
                for (i, p) in self.data.points.iter().enumerate() {
                    let x = frame.point(i as f64, 0.).x - width / 2.;
                    let color = if p.value < 0. {
                        cx.theme().red
                    } else if self.partial {
                        cx.theme().chart_2
                    } else {
                        cx.theme().primary
                    };
                    frame::rect(
                        Bounds::new(
                            point(x, y(0.).min(y(p.value))),
                            size(width, (y(p.value) - y(0.)).abs().max(px(1.))),
                        ),
                        color.opacity(0.85),
                        window,
                    );
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
            .frame(Bounds::new(point(px(0.), px(0.)), bounds.size))?;
        if !frame.bounds.contains(&p) {
            return None;
        }
        let index = (((p.x - frame.bounds.origin.x) / frame.bounds.size.width)
            * self.data.points.len() as f32) as usize;
        let datum = self.data.points.get(index)?;
        Some(TooltipState::new(
            index,
            frame.point(index as f64, datum.value),
            vec![],
        ))
    }
    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let p = self.data.points.get(state.index)?;
        let mut tooltip = Tooltip::new(cursor, bounds.size)
            .plain_row(
                crate::text::translate("native.plots.lag"),
                p.lag.to_string(),
            )
            .plain_row(
                if self.partial { "PACF" } else { "ACF" },
                frame::number(p.value),
            );
        if let (Some(q), Some(probability)) = (p.q_stat, p.p_value) {
            tooltip = tooltip
                .plain_row(format!("Q({})", p.lag), frame::number(q))
                .plain_row("p", frame::number(probability));
        }
        Some(tooltip.into_any_element())
    }
}
