mod axes;
use axes::{AxisDomain, axis_value, chart_box};
use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString, TextAlign,
    Window, fill, point, px, size,
};
use gpui_component::{
    ActiveTheme,
    plot::{
        AxisLabelSide, AxisText, Curve, Grid, IntoPlot, PathCaches, Plot, PlotAxis, TooltipState,
        shape::Line, tooltip::Tooltip,
    },
};
use std::sync::Arc;
use yss_application::chart::{ChartPlotResult, PlotPoint};
use yss_chart_document::ChartType;

pub(super) struct CartesianData {
    pub result: ChartPlotResult,
    pub kind: ChartType,
    x: AxisDomain,
    y: AxisDomain,
}
impl CartesianData {
    pub fn new(result: ChartPlotResult, kind: ChartType) -> Self {
        let x = AxisDomain::from_values(result.data.iter().map(|p| p.x));
        let y = AxisDomain::from_values(result.data.iter().map(|p| p.y));
        Self { result, kind, x, y }
    }
    fn position(&self, datum: &PlotPoint, chart: &Bounds<Pixels>) -> Point<Pixels> {
        point(
            chart.origin.x + chart.size.width * self.x.position(datum.x),
            chart.origin.y + chart.size.height * (1. - self.y.position(datum.y)),
        )
    }
}
#[derive(IntoPlot)]
pub(super) struct CartesianPlot {
    pub data: Arc<CartesianData>,
    pub id: SharedString,
    pub generation: u64,
}
impl Plot for CartesianPlot {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn appear_generation(&self) -> Option<u64> {
        Some(self.generation)
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(mut chart) = chart_box(bounds.size, self.data.result.y_format) else {
            return;
        };
        chart.origin += bounds.origin;
        let color = cx.theme().primary;
        let grid_color = cx.theme().border.opacity(0.65);
        let text_color = cx.theme().muted_foreground;
        let width = chart.size.width.as_f32();
        let height = chart.size.height.as_f32();
        let x_tick_count = ((width
            / if self.data.result.x_format == yss_application::chart::PlotAxisFormat::Datetime {
                150.
            } else {
                90.
            }) as usize)
            .clamp(2, 6);
        let y_tick_count = ((height / 70.) as usize).clamp(2, 6);
        let x_ticks = (0..=x_tick_count)
            .map(|i| i as f32 / x_tick_count as f32 * width)
            .collect::<Vec<_>>();
        let y_ticks = (0..=y_tick_count)
            .map(|i| i as f32 / y_tick_count as f32 * height)
            .collect::<Vec<_>>();
        Grid::new()
            .x(x_ticks.iter().copied().map(px))
            .y(y_ticks.iter().copied().map(px))
            .stroke(grid_color)
            .paint(&chart, window);
        let x_labels = x_ticks.iter().enumerate().map(|(i, position)| {
            AxisText::new(
                axis_value(
                    self.data.x.at(i as f64 / x_tick_count as f64),
                    self.data.result.x_format,
                ),
                px(*position),
                text_color,
            )
            .align(TextAlign::Center)
        });
        let y_labels = y_ticks.iter().enumerate().map(|(i, position)| {
            AxisText::new(
                axis_value(
                    self.data.y.at(1. - i as f64 / y_tick_count as f64),
                    self.data.result.y_format,
                ),
                px(*position),
                text_color,
            )
            .align(TextAlign::Right)
        });
        PlotAxis::new()
            .x(chart.size.height)
            .y(px(0.))
            .y_axis(true)
            .y_label_side(AxisLabelSide::Start)
            .x_label(x_labels)
            .y_label(y_labels)
            .stroke(grid_color)
            .paint(&chart, window, cx);
        if self.data.kind == ChartType::Line {
            let (x, y) = (self.data.x, self.data.y);
            let line = Line::new()
                .data(self.data.result.data.iter().copied())
                .x(move |p: &PlotPoint| Some(x.position(p.x) * width))
                .y(move |p: &PlotPoint| Some((1. - y.position(p.y)) * height))
                .curve(Curve::Linear)
                .stroke(color)
                .stroke_width(px(2.))
                .dot()
                .dot_size(px(4.))
                .dot_fill(color.opacity(0.7));
            let caches = PathCaches::for_paint(self.id.clone(), window, cx);
            caches.update(cx, |caches, _| {
                line.paint_cached(&chart, caches.slot(0), window)
            });
        } else {
            for datum in &self.data.result.data {
                let position = self.data.position(datum, &chart);
                window.paint_quad(
                    fill(
                        Bounds::new(position - point(px(2.5), px(2.5)), size(px(5.), px(5.))),
                        color.opacity(0.7),
                    )
                    .corner_radii(px(2.5)),
                );
            }
        }
    }
    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        let chart = chart_box(bounds.size, self.data.result.y_format)?;
        if !chart.contains(&position) {
            return None;
        }
        let (index, target, distance) = self
            .data
            .result
            .data
            .iter()
            .enumerate()
            .map(|(index, datum)| {
                let target = self.data.position(datum, &chart);
                let dx = (target.x - position.x).as_f32();
                let dy = (target.y - position.y).as_f32();
                (index, target, dx * dx + dy * dy)
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
        let datum = self.data.result.data.get(state.index)?;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title("数据点")
                .plain_row(
                    self.data
                        .result
                        .x_label
                        .as_deref()
                        .unwrap_or("X")
                        .to_owned(),
                    axis_value(datum.x, self.data.result.x_format),
                )
                .plain_row(
                    self.data
                        .result
                        .y_label
                        .as_deref()
                        .unwrap_or("Y")
                        .to_owned(),
                    axis_value(datum.y, self.data.result.y_format),
                )
                .into_any_element(),
        )
    }
}
