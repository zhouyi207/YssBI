use super::*;
use gpui::{App, Div, div, prelude::*, px};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};

fn finite(input: &Entity<InputState>, cx: &App) -> Option<f64> {
    input
        .read(cx)
        .value()
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

impl ResidualView {
    fn range(&self, cx: &App) -> Option<[f64; 2]> {
        let (min, max) = (finite(&self.minimum, cx)?, finite(&self.maximum, cx)?);
        (min <= max).then_some([min, max])
    }
    fn percentage(&self, cx: &App) -> Option<f64> {
        finite(&self.highlight, cx).filter(|value| (0.0..=100.0).contains(value))
    }
    pub(super) fn controls(&self, cx: &mut Context<Self>) -> Div {
        let highlight_available = self
            .data
            .as_ref()
            .is_some_and(|data| data.highlight_available);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .items_end()
                    .child(
                        Button::new("residual-mode")
                            .small()
                            .ghost()
                            .disabled(self.loading)
                            .label(crate::text::translate(if self.selection.adjacent {
                                "native.reports.adjacentResiduals"
                            } else {
                                "native.reports.fittedResiduals"
                            }))
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.loading {
                                    return;
                                }
                                view.selection.adjacent = !view.selection.adjacent;
                                view.reset_range(window, cx);
                                view.load(window, cx);
                            })),
                    )
                    .child(field(
                        "native.reports.highlightPercent",
                        &self.highlight,
                        self.loading || !highlight_available,
                    ))
                    .child(
                        Button::new("apply-leverage")
                            .small()
                            .ghost()
                            .label(crate::text::translate("native.reports.applyHighlight"))
                            .disabled(
                                self.loading
                                    || !highlight_available
                                    || self.percentage(cx).is_none(),
                            )
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.loading {
                                    return;
                                }
                                if let Some(value) = view.percentage(cx) {
                                    view.selection.highlight = value;
                                    view.load(window, cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .items_end()
                    .child(field("native.reports.minX", &self.minimum, self.loading))
                    .child(field("native.reports.maxX", &self.maximum, self.loading))
                    .child(
                        Button::new("apply-residual-range")
                            .small()
                            .ghost()
                            .label(crate::text::translate("native.reports.applyRange"))
                            .disabled(self.loading || self.range(cx).is_none())
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.loading {
                                    return;
                                }
                                if let Some(range) = view.range(cx) {
                                    view.selection.range = Some(range);
                                    view.load(window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("reset-residual-range")
                            .small()
                            .ghost()
                            .label(crate::text::translate("native.reports.resetRange"))
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.loading {
                                    return;
                                }
                                view.reset_range(window, cx);
                                view.load(window, cx);
                            })),
                    ),
            )
    }
}

fn field(label: &str, input: &Entity<InputState>, disabled: bool) -> Div {
    div()
        .w(px(155.))
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().child(crate::text::translate(label)))
        .child(Input::new(input).small().disabled(disabled))
}
