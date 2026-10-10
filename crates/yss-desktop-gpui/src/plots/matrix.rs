use super::{
    axes::AxisDomain,
    frame::{self, Axis, Frame},
};
use gpui_kit::component::{
    ActiveTheme, Colorize,
    plot::{IntoPlot, Plot, PlotLabel, TooltipState, label::Text, tooltip::Tooltip},
};
use gpui_kit::{
    AnyElement, App, Bounds, ElementId, Hsla, IntoElement, Pixels, Point, SharedString, TextAlign,
    Window, point, px, size,
};
use std::sync::Arc;
use yss_application::graph::results::plot::{CorrelationPlot, HeatmapPlot};

pub(crate) struct MatrixData {
    x: Vec<String>,
    y: Vec<String>,
    cells: Vec<Vec<Option<f64>>>,
    p_values: Option<Vec<Vec<Option<f64>>>>,
    range: [f64; 2],
}
impl MatrixData {
    pub fn correlation(plot: CorrelationPlot) -> Self {
        Self {
            x: plot.labels.clone(),
            y: plot.labels,
            cells: plot.matrix,
            p_values: Some(plot.p_matrix),
            range: [-1., 1.],
        }
    }
    pub fn heatmap(plot: HeatmapPlot) -> Self {
        let range = plot
            .matrix
            .iter()
            .flatten()
            .fold([f64::INFINITY, f64::NEG_INFINITY], |[min, max], &v| {
                [min.min(v), max.max(v)]
            });
        Self {
            x: plot.x_labels,
            y: plot.y_labels,
            cells: plot
                .matrix
                .into_iter()
                .map(|row| row.into_iter().map(Some).collect())
                .collect(),
            p_values: None,
            range,
        }
    }
    fn frame(&self, bounds: Bounds<Pixels>) -> Option<Frame<'_>> {
        let mut frame = Frame::new(
            bounds,
            112.,
            48.,
            38.,
            Axis::Labels(&self.x),
            Axis::Labels(&self.y),
        )?;
        if self.p_values.is_some() {
            let side = frame.bounds.size.width.min(frame.bounds.size.height);
            frame.bounds.origin += point(
                (frame.bounds.size.width - side) / 2.,
                (frame.bounds.size.height - side) / 2.,
            );
            frame.bounds.size = size(side, side);
        }
        Some(frame)
    }
    fn color(&self, fraction: f32, cx: &App) -> Hsla {
        let t = if self.p_values.is_some() {
            fraction
        } else {
            1. - fraction
        };
        let endpoint = if t < 0.5 {
            cx.theme().red
        } else {
            cx.theme().blue
        };
        // Colorize weights the receiver, rather than the target, by this factor.
        endpoint.mix_oklab(cx.theme().muted, (t * 2. - 1.).abs())
    }
    fn fraction(&self, value: f64) -> f32 {
        if self.range[0] == self.range[1] {
            0.5
        } else {
            AxisDomain::fixed(self.range).position(value).clamp(0., 1.)
        }
    }
}
#[derive(IntoPlot)]
pub(crate) struct MatrixPlot {
    pub data: Arc<MatrixData>,
    pub id: SharedString,
}
impl Plot for MatrixPlot {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(frame) = self.data.frame(bounds) else {
            return;
        };
        frame.axes(window, cx);
        let cell = size(
            frame.bounds.size.width / self.data.x.len() as f32,
            frame.bounds.size.height / self.data.y.len() as f32,
        );
        for (r, row) in self.data.cells.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                if let Some(value) = value {
                    let inset = if self.data.p_values.is_some() {
                        0.025
                    } else {
                        0.0075
                    };
                    frame::rect(
                        Bounds::new(
                            frame.bounds.origin
                                + point(
                                    cell.width * (c as f32 + inset),
                                    cell.height * (r as f32 + inset),
                                ),
                            size(
                                cell.width * (1. - 2. * inset),
                                cell.height * (1. - 2. * inset),
                            ),
                        ),
                        self.data.color(self.data.fraction(*value), cx),
                        window,
                    );
                }
            }
        }
        for i in 0..64 {
            frame::rect(
                Bounds::new(
                    frame.bounds.origin + point(frame.bounds.size.width * i as f32 / 64., px(-12.)),
                    size(frame.bounds.size.width / 64. + px(0.1), px(6.)),
                ),
                self.data.color(i as f32 / 63., cx),
                window,
            );
        }
        PlotLabel::new(vec![
            Text::new(
                frame::number(self.data.range[0]),
                point(px(0.), px(-28.)),
                cx.theme().muted_foreground,
            ),
            Text::new(
                frame::number(self.data.range[1]),
                point(frame.bounds.size.width, px(-28.)),
                cx.theme().muted_foreground,
            )
            .align(TextAlign::Right),
        ])
        .paint(&frame.bounds, window, cx);
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
        if !frame.bounds.contains(&position) {
            return None;
        }
        let c = (((position.x - frame.bounds.origin.x) / frame.bounds.size.width)
            * self.data.x.len() as f32) as usize;
        let r = (((position.y - frame.bounds.origin.y) / frame.bounds.size.height)
            * self.data.y.len() as f32) as usize;
        self.data.cells.get(r)?.get(c)?.as_ref()?;
        Some(TooltipState::new(
            r * self.data.x.len() + c,
            position,
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
        let (r, c) = (
            state.index / self.data.x.len(),
            state.index % self.data.x.len(),
        );
        let value = self.data.cells.get(r)?.get(c)?.as_ref()?;
        let mut tooltip = Tooltip::new(cursor, bounds.size)
            .plain_row(
                crate::text::translate("native.plots.row"),
                self.data.y[r].clone(),
            )
            .plain_row(
                crate::text::translate("native.plots.column"),
                self.data.x[c].clone(),
            )
            .plain_row(
                crate::text::translate(if self.data.p_values.is_some() {
                    "native.plots.correlation"
                } else {
                    "native.plots.value"
                }),
                frame::number(*value),
            );
        if let Some(values) = &self.data.p_values {
            tooltip = tooltip.plain_row(
                "p",
                values[r][c]
                    .map(frame::number)
                    .unwrap_or_else(|| crate::text::translate("native.plots.unavailable")),
            );
        }
        Some(tooltip.into_any_element())
    }
}
