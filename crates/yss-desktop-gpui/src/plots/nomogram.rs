use super::frame;
use gpui_kit::component::{
    ActiveTheme,
    plot::{
        IntoPlot, Plot, PlotLabel, TooltipState,
        label::{Text, truncate_text_to_width},
        tooltip::Tooltip,
    },
};
use gpui_kit::{
    AnyElement, App, Bounds, ElementId, IntoElement, Pixels, Point, SharedString, TextAlign,
    Window, point, px,
};
use std::sync::Arc;
use yss_application::graph::results::plot::NomogramAxis;
pub(crate) struct NomogramData {
    axes: Vec<NomogramAxis>,
    offsets: Vec<usize>,
}
impl NomogramData {
    pub fn new(mut axes: Vec<NomogramAxis>) -> Self {
        let mut offset = 0;
        let offsets = axes
            .iter_mut()
            .map(|axis| {
                axis.ticks.sort_by(|a, b| a.position.total_cmp(&b.position));
                let start = offset;
                offset += axis.ticks.len();
                start
            })
            .collect();
        Self { axes, offsets }
    }
    pub fn height(&self) -> f32 {
        self.axes.len() as f32 * 82. + 24.
    }
}
#[derive(IntoPlot)]
pub(crate) struct Nomogram {
    pub data: Arc<NomogramData>,
    pub id: SharedString,
}
impl Plot for Nomogram {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone().into())
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let span = bounds.size.width - px(80.);
        if span <= px(0.) {
            return;
        }
        let mut labels = Vec::new();
        for (row, axis) in self.data.axes.iter().enumerate() {
            let y = px(row as f32 * 82. + 40.);
            let x = |position: f64| px(40.) + span * position as f32;
            let title =
                truncate_text_to_width(&axis.label.clone().into(), px(12.), span.as_f32(), window);
            labels.push(
                Text::new(title, point(px(40.), y - px(22.)), cx.theme().foreground)
                    .font_size(px(12.)),
            );
            if let (Some(first), Some(last)) = (axis.ticks.first(), axis.ticks.last()) {
                frame::line(
                    bounds.origin + point(x(first.position), y),
                    bounds.origin + point(x(last.position), y),
                    1.,
                    cx.theme().border,
                    window,
                );
            }
            let mut previous = f32::NEG_INFINITY;
            for tick in &axis.ticks {
                let position = x(tick.position);
                frame::line(
                    bounds.origin + point(position, y - px(4.)),
                    bounds.origin + point(position, y + px(5.)),
                    1.,
                    if row == 0 {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    },
                    window,
                );
                if position.as_f32() - previous >= 60. {
                    labels.push(
                        Text::new(
                            tick.label.clone(),
                            point(position, y + px(10.)),
                            cx.theme().muted_foreground,
                        )
                        .align(TextAlign::Center),
                    );
                    previous = position.as_f32();
                }
            }
        }
        PlotLabel::new(labels).paint(&bounds, window, cx);
    }
    fn tooltip_state(
        &self,
        p: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        let row = (p.y.as_f32() / 82.) as usize;
        let axis = self.data.axes.get(row)?;
        let span = bounds.size.width - px(80.);
        let (index, tick) = axis.ticks.iter().enumerate().min_by(|a, b| {
            (px(40.) + span * a.1.position as f32 - p.x)
                .abs()
                .partial_cmp(&(px(40.) + span * b.1.position as f32 - p.x).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
        let target = point(
            px(40.) + span * tick.position as f32,
            px(row as f32 * 82. + 40.),
        );
        ((target.x - p.x).abs() < px(20.)).then_some(TooltipState::new(
            self.data.offsets[row] + index,
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
        let row = self
            .data
            .offsets
            .partition_point(|offset| *offset <= state.index)
            .checked_sub(1)?;
        let axis = &self.data.axes[row];
        let tick = axis.ticks.get(state.index - self.data.offsets[row])?;
        Some(
            Tooltip::new(cursor, bounds.size)
                .title(axis.label.clone())
                .plain_row(
                    crate::text::translate("native.plots.value"),
                    tick.label.clone(),
                )
                .into_any_element(),
        )
    }
}
