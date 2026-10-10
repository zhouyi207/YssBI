//! The editor hosts plugin details; sidebar and modal composition reuse the same owner.
use super::PluginsPanel;
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::ActiveTheme;

impl Render for PluginsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("native-plugins")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h_0().child(self.render_details(cx)))
            .child(self.render_status(cx))
    }
}
impl PluginsPanel {
    pub(super) fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_xs()
            .text_color(if self.error.is_some() {
                cx.theme().danger
            } else {
                cx.theme().muted_foreground
            })
            .child(
                self.error
                    .clone()
                    .or_else(|| self.task.map(str::to_owned))
                    .or_else(|| self.feedback.clone())
                    .unwrap_or_else(|| {
                        crate::text::format(
                            "native.plugins.installedCount",
                            &[("value0", self.entries.len().to_string())],
                        )
                    }),
            )
    }
}
