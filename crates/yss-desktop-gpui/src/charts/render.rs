use super::{
    ChartEditor, SaveChart,
    details::chart_type_label,
    query::{PreviewData, PreviewFailure},
};
use crate::{
    appearance,
    plots::{cartesian::CartesianPlot, histogram},
};
use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit_assets::IconName;

impl Render for ChartEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chart-editor")
            .key_context("ChartEditor")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|view, _: &SaveChart, window, cx| view.save(window, cx)))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|view, _, window, cx| view.focus_chart(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Icon::new(IconName::ChartLine).size_4())
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(chart_type_label(self.draft.chart_type)),
                    )
                    .child(
                        Button::new("chart-refresh")
                            .small()
                            .ghost()
                            .label(crate::text::t("common.refresh"))
                            .disabled(self.busy())
                            .on_click(cx.listener(|view, _, window, cx| view.refresh(window, cx))),
                    )
                    .child(
                        Button::new("chart-save")
                            .small()
                            .primary()
                            .label(crate::text::t("common.save"))
                            .disabled(self.busy() || !self.available || !self.dirty())
                            .on_click(cx.listener(|view, _, window, cx| view.save(window, cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .relative()
                    .child(self.render_preview(cx)),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
    }
}
impl ChartEditor {
    fn render_preview_failure(
        &self,
        failure: &PreviewFailure,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("chart-preview-error")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .p_4()
            .role(gpui::Role::Alert)
            .child(
                Icon::new(IconName::TriangleAlert)
                    .size_6()
                    .text_color(cx.theme().danger),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(failure.summary()),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::translate("common.errorCode"))
                    .child(failure.code()),
            )
            .child(
                Button::new("chart-preview-retry")
                    .small()
                    .outline()
                    .label(crate::text::translate("common.retry"))
                    .disabled(self.busy() || !self.available)
                    .on_click(cx.listener(|view, _, window, cx| view.refresh(window, cx))),
            )
            .into_any_element()
    }
    fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.preview_loading || self.reading {
            return appearance::empty_state(
                IconName::ChartLine,
                crate::text::t("native.charts.loading"),
                crate::text::t("native.charts.preparingData"),
                cx,
            )
            .into_any_element();
        }
        let Some(preview) = &self.preview else {
            return appearance::empty_state(
                IconName::ChartLine,
                crate::text::t("native.charts.previewUnavailable"),
                crate::text::t("native.charts.refreshHint"),
                cx,
            )
            .into_any_element();
        };
        match preview.as_ref() {
            PreviewData::Empty(message) => appearance::empty_state(
                IconName::ChartLine,
                crate::text::translate("chart.previewEmpty"),
                crate::text::translate(message),
                cx,
            )
            .into_any_element(),
            PreviewData::Failed(failure) => self.render_preview_failure(failure, cx),
            PreviewData::Histogram {
                bins,
                column,
                other_count,
            } => {
                if bins.is_empty() {
                    return appearance::empty_state(
                        IconName::ChartLine,
                        crate::text::t("native.charts.noDistribution"),
                        crate::text::t("native.charts.chooseValidColumn"),
                        cx,
                    )
                    .into_any_element();
                }
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .p_4()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(crate::text::format(
                                "native.charts.frequency",
                                &[("column", column.to_string())],
                            )),
                    )
                    .child(div().flex_1().min_h_0().child(histogram::render(
                        format!("chart-bars-{}", cx.entity_id()),
                        bins,
                        cx,
                    )))
                    .when(*other_count > 0, |view| {
                        view.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(crate::text::format(
                                    "native.charts.otherValues",
                                    &[("other_count", other_count.to_string())],
                                )),
                        )
                    })
                    .into_any_element()
            }
            PreviewData::Cartesian(data) => div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::format(
                            "native.charts.plotSummary",
                            &[
                                (
                                    "value0",
                                    data.result.x_label.as_deref().unwrap_or("X").to_string(),
                                ),
                                (
                                    "value1",
                                    data.result.y_label.as_deref().unwrap_or("Y").to_string(),
                                ),
                                ("value2", data.result.data.len().to_string()),
                            ],
                        )),
                )
                .child(div().flex_1().min_h_0().child(CartesianPlot {
                    data: data.clone(),
                    id: format!("chart-points-{}", cx.entity_id()).into(),
                    generation: self.preview_generation,
                    show_points: true,
                }))
                .child(div().h(px(8.)))
                .into_any_element(),
        }
    }
}
