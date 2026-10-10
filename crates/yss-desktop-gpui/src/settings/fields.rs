//! Shared label, description and control layout for every settings category.
use gpui_kit::component::ActiveTheme;
use gpui_kit::{AnyElement, App, IntoElement, div, prelude::*, px};

impl crate::settings::SettingsPanel {
    pub(in crate::settings) fn render_field(
        &self,
        label: &str,
        description: &str,
        control: impl IntoElement,
        cx: &App,
    ) -> AnyElement {
        let compact = self.render_width <= 720.;
        let control_width = if self.render_width <= 900. {
            240.
        } else {
            280.
        };
        div()
            .flex()
            .when(compact, |view| view.flex_col())
            .flex_wrap()
            .items_start()
            .gap_4()
            .py_5()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .when(compact, |view| view.w_full())
                    .when(!compact, |view| view.flex_1().min_w(px(200.)))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(div().text_sm().child(label.to_owned()))
                    .when(!description.is_empty(), |view| {
                        view.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(description.to_owned()),
                        )
                    }),
            )
            .child(
                div()
                    .when(compact, |view| view.w_full())
                    .when(!compact, |view| view.w(px(control_width)))
                    .max_w_full()
                    .flex_shrink_0()
                    .child(control),
            )
            .into_any_element()
    }
}
