use super::axes::{AxisDomain, axis_value, chart_box};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString, TextAlign,
    Window, point, px,
};
use gpui_component::{
    ActiveTheme,
    plot::{
        AxisLabelSide, AxisText, Grid, IntoPlot, Plot, PlotAxis, TooltipState, tooltip::Tooltip,
    },
};
use std::sync::Arc;
mod marks;
use yss_application::chart::{ChartPlotResult, PlotPoint};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CartesianKind {
    Scatter,
    Line,
    Ecdf,
    Density,
}
pub(crate) struct ScatterObservation {
    pub number: usize,
    pub highlighted: bool,
}
pub(crate) struct CartesianOptions {
    pub kind: CartesianKind,
    pub reference_lines: Vec<[PlotPoint; 2]>,
    pub point_sizes: Option<Vec<f64>>,
    pub x_domain: Option<[f64; 2]>,
    pub y_domain: Option<[f64; 2]>,
    pub observations: Option<Vec<ScatterObservation>>,
    pub zero_line: bool,
    pub symmetric_y: bool,
    pub x_min: Option<f64>,
}
impl CartesianOptions {
    pub fn new(kind: CartesianKind) -> Self {
        Self {
            kind,
            reference_lines: Vec::new(),
            point_sizes: None,
            x_domain: None,
            y_domain: None,
            observations: None,
            zero_line: false,
            symmetric_y: false,
            x_min: None,
        }
    }
}

pub(crate) struct CartesianData {
    pub result: ChartPlotResult,
    pub kind: CartesianKind,
    reference_lines: Vec<[PlotPoint; 2]>,
    point_sizes: Option<Vec<f64>>,
    largest_size: f64,
    observations: Option<Vec<ScatterObservation>>,
    zero_line: bool,
    x: AxisDomain,
    y: AxisDomain,
}
impl CartesianData {
    pub fn new(result: ChartPlotResult, options: CartesianOptions) -> Self {
        let x = options.x_domain.map(AxisDomain::fixed).unwrap_or_else(|| {
            AxisDomain::from_values(
                result
                    .data
                    .iter()
                    .map(|p| p.x)
                    .chain(options.reference_lines.iter().flatten().map(|p| p.x)),
            )
        });
        let x = options.x_min.map_or(x, |min| {
            AxisDomain::fixed([min, x.at(1.).clamp(min + 0.01, f64::MAX)])
        });
        let natural_y = AxisDomain::from_values(
            result
                .data
                .iter()
                .map(|p| p.y)
                .chain(options.reference_lines.iter().flatten().map(|p| p.y)),
        );
        let y = options
            .y_domain
            .map(AxisDomain::fixed)
            .unwrap_or_else(|| match options.kind {
                CartesianKind::Ecdf => AxisDomain::fixed([0., 1.]),
                CartesianKind::Density => {
                    AxisDomain::fixed([0., natural_y.at(1.).clamp(0.01, f64::MAX)])
                }
                _ => natural_y,
            });
        let y = if options.symmetric_y {
            let extent = y
                .at(0.)
                .abs()
                .max(y.at(1.).abs())
                .clamp(f64::MIN_POSITIVE, f64::MAX);
            AxisDomain::fixed([-extent, extent])
        } else {
            y
        };
        let largest_size = options
            .point_sizes
            .as_ref()
            .map_or(0., |values| values.iter().copied().fold(0_f64, f64::max));
        Self {
            result,
            kind: options.kind,
            x,
            y,
            reference_lines: options.reference_lines,
            point_sizes: options.point_sizes,
            largest_size,
            observations: options.observations,
            zero_line: options.zero_line,
        }
    }
    fn radius(&self, index: usize) -> f32 {
        self.point_sizes.as_ref().map_or(2.5, |sizes| {
            if self.largest_size > 0. {
                (sizes[index] / self.largest_size).sqrt() as f32 * 16.
            } else {
                0.
            }
        })
    }
    fn highlighted(&self, index: usize) -> bool {
        self.observations
            .as_ref()
            .and_then(|rows| rows.get(index))
            .is_some_and(|row| row.highlighted)
    }
    fn position(&self, datum: &PlotPoint, chart: &Bounds<Pixels>) -> Point<Pixels> {
        point(
            chart.origin.x + chart.size.width * self.x.position(datum.x),
            chart.origin.y + chart.size.height * (1. - self.y.position(datum.y)),
        )
    }
}
#[derive(IntoPlot)]
pub(crate) struct CartesianPlot {
    pub data: Arc<CartesianData>,
    pub id: SharedString,
    pub generation: u64,
    pub show_points: bool,
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
        window.with_content_mask(Some(gpui::ContentMask { bounds: chart }), |window| {
            self.paint_marks(&chart, window, cx);
        });
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
            .filter(|(index, _)| self.data.radius(*index) > 0.)
            .map(|(index, datum)| {
                let target = self.data.position(datum, &chart);
                let dx = (target.x - position.x).as_f32();
                let dy = (target.y - position.y).as_f32();
                (index, target, dx * dx + dy * dy)
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))?;
        (distance <= self.data.radius(index).max(10.).powi(2)).then_some(TooltipState::new(
            index,
            target,
            vec![target],
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
        let datum = self.data.result.data.get(state.index)?;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(crate::text::translate("native.charts.dataPoints"))
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
                .when_some(self.data.point_sizes.as_ref(), |tooltip, sizes| {
                    tooltip.plain_row(
                        crate::text::translate("native.plots.bubbleSize"),
                        axis_value(
                            sizes[state.index],
                            yss_application::chart::PlotAxisFormat::Number,
                        ),
                    )
                })
                .when_some(
                    self.data
                        .observations
                        .as_ref()
                        .and_then(|rows| rows.get(state.index)),
                    |tooltip, observation| {
                        tooltip
                            .plain_row(
                                crate::text::translate("native.reports.observation"),
                                observation.number.to_string(),
                            )
                            .when(observation.highlighted, |tooltip| {
                                tooltip.plain_row(
                                    crate::text::translate("native.reports.highLeverage"),
                                    crate::text::translate("native.reports.yes"),
                                )
                            })
                    },
                )
                .into_any_element(),
        )
    }
}
