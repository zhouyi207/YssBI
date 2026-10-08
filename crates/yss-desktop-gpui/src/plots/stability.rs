//! Equal-scale complex roots with a unit-circle reference; no stability inference.
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString, Window, point,
    px, size,
};
use gpui_component::{
    ActiveTheme,
    plot::{IntoPlot, Plot, TooltipState, tooltip::Tooltip},
};
use yss_application::graph::results::report::structured::StabilityPoint;

use super::{
    axes::AxisDomain,
    frame::{Axis, Frame, dot, line, number},
};

pub(crate) struct StabilityData {
    roots: Vec<StabilityPoint>,
    domain: AxisDomain,
}

impl StabilityData {
    pub fn new(roots: Vec<StabilityPoint>) -> Self {
        let extent = roots
            .iter()
            .flat_map(|root| [root.re.abs(), root.im.abs()])
            .fold(1.2_f64, f64::max);
        let extent = (extent * 1.15).min(f64::MAX);
        Self {
            roots,
            domain: AxisDomain::fixed([-extent, extent]),
        }
    }

    fn frame(&self, bounds: Bounds<Pixels>) -> Option<Frame<'_>> {
        let mut frame = Frame::new(
            bounds,
            70.,
            20.,
            38.,
            Axis::Numeric(self.domain),
            Axis::Numeric(self.domain),
        )?;
        let side = frame.bounds.size.width.min(frame.bounds.size.height);
        frame.bounds.origin += point(
            (frame.bounds.size.width - side) / 2.,
            (frame.bounds.size.height - side) / 2.,
        );
        frame.bounds.size = size(side, side);
        Some(frame)
    }
}

#[derive(IntoPlot)]
pub(crate) struct StabilityPlot {
    pub data: Arc<StabilityData>,
    pub id: SharedString,
    pub page: usize,
}

impl Plot for StabilityPlot {
    fn id(&self) -> Option<ElementId> {
        Some((self.id.clone(), self.page).into())
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(frame) = self.data.frame(bounds) else {
            return;
        };
        frame.axes(window, cx);
        let origin = frame.point(0., 0.);
        let radius = (frame.point(1., 0.).x - origin.x).abs();
        window.paint_quad(
            gpui::outline(
                Bounds::new(
                    origin - point(radius, radius),
                    size(radius * 2., radius * 2.),
                ),
                cx.theme().primary,
                gpui::BorderStyle::default(),
            )
            .corner_radii(radius),
        );
        let min = self.data.domain.at(0.);
        let max = self.data.domain.at(1.);
        line(
            frame.point(min, 0.),
            frame.point(max, 0.),
            1.,
            cx.theme().muted_foreground,
            window,
        );
        line(
            frame.point(0., min),
            frame.point(0., max),
            1.,
            cx.theme().muted_foreground,
            window,
        );
        for root in &self.data.roots {
            dot(
                frame.point(root.re, root.im),
                4.,
                cx.theme().primary,
                window,
            );
        }
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        let frame = self
            .data
            .frame(Bounds::new(point(px(0.), px(0.)), bounds.size))?;
        let (index, target, distance) = self
            .data
            .roots
            .iter()
            .enumerate()
            .map(|(index, root)| {
                let target = frame.point(root.re, root.im);
                let delta = target - position;
                (
                    index,
                    target,
                    delta.x.as_f32().powi(2) + delta.y.as_f32().powi(2),
                )
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))?;
        (distance <= 100.).then_some(TooltipState::new(index, target, vec![target]))
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let root = self.data.roots.get(state.index)?;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(crate::text::translate("native.reports.root"))
                .plain_row(
                    crate::text::translate("native.reports.real"),
                    number(root.re),
                )
                .plain_row(
                    crate::text::translate("native.reports.imaginary"),
                    number(root.im),
                )
                .plain_row(
                    crate::text::translate("native.reports.modulus"),
                    root.modulus.map(number).unwrap_or_else(|| "—".into()),
                )
                .into_any_element(),
        )
    }
}
