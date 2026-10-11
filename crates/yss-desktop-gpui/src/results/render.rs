//! Native result content shares one toolbar and keeps report entities across mode changes.
use super::*;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    tooltip::Tooltip,
};
use gpui_kit::{Render, px, uniform_list};

impl Render for ResultPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = crate::text::locale();
        if self.view_locale != locale {
            self.view_locale = locale;
            if let Some(value) = &self.value {
                self.rows = value::rows(value, &self.expanded, self.tables);
            }
        }
        div()
            .id("result-view")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(self.toolbar(cx))
            .child(self.read_feedback(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .when_some(
                        self.report.as_ref().filter(|_| self.report_mode),
                        |body, report| body.child(report.clone()),
                    )
                    .when_some(
                        self.table.as_ref().filter(|_| !self.report_mode),
                        |body, table| body.child(super::table::present(table, cx)),
                    )
                    .when_some(self.plot.as_ref(), |body, plot| body.child(plot.clone()))
                    .when(
                        !self.report_mode && self.table.is_none() && self.plot.is_none(),
                        |body| {
                            body.child(
                                uniform_list(
                                    "result-values",
                                    self.rows.len(),
                                    cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                                        range.map(|index| view.render_row(index, cx)).collect()
                                    }),
                                )
                                .size_full(),
                            )
                        },
                    ),
            )
    }
}

impl ResultPanel {
    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let row = &self.rows[index];
        let path = row.path.clone();
        let part = row.table.clone();
        let text: gpui_kit::SharedString = if part.is_some() {
            crate::text::translate("native.results.viewData").into()
        } else {
            row.value.clone().into()
        };
        div()
            .h(px(32.))
            .flex()
            .items_center()
            .gap_2()
            .pl(px(row.depth as f32 * 16. + 8.))
            .border_b_1()
            .border_color(cx.theme().border)
            .text_sm()
            .child(
                Button::new(("value-row", index))
                    .small()
                    .ghost()
                    .label(row.label.clone())
                    .tooltip(row.label.clone())
                    .disabled(!row.expandable && part.is_none())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if let Some(part) = &part {
                            view.load_page(Some(part.clone()), 0, window, cx);
                        } else {
                            if !view.expanded.insert(path.clone()) {
                                view.expanded.remove(&path);
                            }
                            if let Some(value) = &view.value {
                                view.rows = value::rows(value, &view.expanded, view.tables);
                            }
                            cx.notify();
                        }
                    })),
            )
            .child(
                div()
                    .id(("result-value", index))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(text.clone())
                    .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx)),
            )
    }
}
