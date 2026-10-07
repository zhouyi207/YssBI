//! Settings field presentation, shared by provider and model drafts.
use gpui::{AnyElement, App, IntoElement, div, prelude::*, px};
use gpui_component::ActiveTheme;

pub(in crate::settings) fn field(
    label: &str,
    description: &str,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_start()
        .gap_4()
        .py_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .w(px(210.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_sm().child(label.to_owned()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(description.to_owned()),
                ),
        )
        .child(div().flex_1().min_w(px(220.)).child(control))
        .into_any_element()
}
