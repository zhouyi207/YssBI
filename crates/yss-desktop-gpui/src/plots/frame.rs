//! Axes and axis-aligned marks shared by statistical plots.
use super::axes::{AxisDomain, axis_value};
use gpui_kit::component::{
    ActiveTheme,
    plot::{AxisLabelSide, AxisText, Grid, PlotAxis, label::truncate_text_to_width},
};
use gpui_kit::{App, Bounds, Hsla, Pixels, Point, TextAlign, Window, fill, point, px, size};
use yss_application::chart::PlotAxisFormat;

#[derive(Clone, Copy)]
pub(super) enum Axis<'a> {
    Numeric(AxisDomain),
    Labels(&'a [String]),
}
impl Axis<'_> {
    fn fraction(self, value: f64, vertical: bool) -> f32 {
        match self {
            Self::Numeric(domain) => {
                if vertical {
                    1. - domain.position(value)
                } else {
                    domain.position(value)
                }
            }
            Self::Labels(labels) => (value as f32 + 0.5) / labels.len() as f32,
        }
    }
    fn ticks(
        self,
        length: Pixels,
        vertical: bool,
        color: Hsla,
        window: &mut Window,
    ) -> Vec<AxisText> {
        match self {
            Self::Numeric(domain) => {
                let count =
                    ((length.as_f32() / if vertical { 55. } else { 90. }) as usize).clamp(2, 6);
                (0..=count)
                    .map(|i| {
                        let fraction = i as f64 / count as f64;
                        AxisText::new(
                            number(domain.at(if vertical { 1. - fraction } else { fraction })),
                            length * fraction as f32,
                            color,
                        )
                        .align(if vertical {
                            TextAlign::Right
                        } else {
                            TextAlign::Center
                        })
                    })
                    .collect()
            }
            Self::Labels(labels) => {
                let capacity =
                    (length.as_f32() / if vertical { 20. } else { 70. }).max(1.) as usize;
                let step = labels.len().div_ceil(capacity).max(1);
                labels
                    .iter()
                    .enumerate()
                    .step_by(step)
                    .map(|(i, label)| {
                        let text = truncate_text_to_width(
                            &label.replace(['\r', '\n'], " ").into(),
                            px(10.),
                            if vertical { 100. } else { 65. },
                            window,
                        );
                        AxisText::new(text, length * self.fraction(i as f64, vertical), color)
                            .align(if vertical {
                                TextAlign::Right
                            } else {
                                TextAlign::Center
                            })
                    })
                    .collect()
            }
        }
    }
}

pub(super) struct Frame<'a> {
    pub bounds: Bounds<Pixels>,
    pub x: Axis<'a>,
    pub y: Axis<'a>,
}
impl<'a> Frame<'a> {
    pub fn new(
        bounds: Bounds<Pixels>,
        left: f32,
        top: f32,
        bottom: f32,
        x: Axis<'a>,
        y: Axis<'a>,
    ) -> Option<Self> {
        let width = bounds.size.width - px(left + 24.);
        let height = bounds.size.height - px(top + bottom);
        (width > px(24.) && height > px(24.)).then_some(Self {
            bounds: Bounds::new(
                bounds.origin + point(px(left), px(top)),
                size(width, height),
            ),
            x,
            y,
        })
    }
    pub fn point(&self, x: f64, y: f64) -> Point<Pixels> {
        point(
            self.bounds.origin.x + self.bounds.size.width * self.x.fraction(x, false),
            self.bounds.origin.y + self.bounds.size.height * self.y.fraction(y, true),
        )
    }
    pub fn axes(&self, window: &mut Window, cx: &mut App) {
        let color = cx.theme().muted_foreground;
        let x = self.x.ticks(self.bounds.size.width, false, color, window);
        let y = self.y.ticks(self.bounds.size.height, true, color, window);
        if matches!(self.y, Axis::Numeric(_)) {
            Grid::new()
                .y(y.iter().map(|label| label.tick))
                .stroke(cx.theme().border.opacity(0.5))
                .paint(&self.bounds, window);
        }
        PlotAxis::new()
            .x(self.bounds.size.height)
            .y(px(0.))
            .y_axis(true)
            .y_label_side(AxisLabelSide::Start)
            .x_label(x)
            .y_label(y)
            .stroke(cx.theme().border)
            .paint(&self.bounds, window, cx);
    }
    pub fn right_axis(&self, domain: AxisDomain, percent: bool, window: &mut Window, cx: &mut App) {
        let axis = Axis::Numeric(domain);
        let labels = axis
            .ticks(
                self.bounds.size.height,
                true,
                cx.theme().muted_foreground,
                window,
            )
            .into_iter()
            .map(|mut label| {
                label.align = TextAlign::Left;
                if percent {
                    let fraction = (label.tick / self.bounds.size.height) as f64;
                    label.text = format!("{:.0}%", domain.at(1. - fraction) * 100.).into();
                }
                label
            });
        PlotAxis::new()
            .y(self.bounds.size.width)
            .y_axis(true)
            .y_label(labels)
            .stroke(cx.theme().border)
            .paint(&self.bounds, window, cx);
    }
}
pub(super) fn number(value: f64) -> String {
    axis_value(value, PlotAxisFormat::Number)
}
pub(super) fn rect(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    window.paint_quad(fill(bounds, color));
}
pub(super) fn line(
    a: Point<Pixels>,
    b: Point<Pixels>,
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
    // Statistical intervals, whiskers and reference lines are axis-aligned.
    let thickness = px(width);
    rect(
        Bounds::new(
            point(a.x.min(b.x) - thickness / 2., a.y.min(b.y) - thickness / 2.),
            size(
                (a.x - b.x).abs().max(thickness),
                (a.y - b.y).abs().max(thickness),
            ),
        ),
        color,
        window,
    );
}
pub(super) fn dot(p: Point<Pixels>, radius: f32, color: Hsla, window: &mut Window) {
    let radius = px(radius);
    window.paint_quad(
        fill(
            Bounds::new(p - point(radius, radius), size(radius * 2., radius * 2.)),
            color,
        )
        .corner_radii(radius),
    );
}
