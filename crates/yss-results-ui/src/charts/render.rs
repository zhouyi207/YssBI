use super::{ChartEditor, SaveChart, details::chart_type_label, query::PreviewData};
use crate::plots::{cartesian::CartesianPlot, histogram};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use yss_resource_ui::file_session::FileEditor;
use yss_ui::appearance;

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
                gpui_kit::MouseButton::Left,
                cx.listener(|view, _, window, cx| view.focus_chart(window, cx)),
            )
            .child(
                div()
                    .h(appearance::HEADER_HEIGHT)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Icon::new(IconName::ChartLine).size_4())
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(chart_type_label(self.draft.chart_type)),
                    )
                    .child(
                        Button::new("chart-refresh")
                            .small()
                            .ghost()
                            .label(yss_ui::text::t("common.refresh"))
                            .disabled(self.busy() || self.file.removed())
                            .on_click(cx.listener(|view, _, window, cx| view.reload(window, cx))),
                    )
                    .child(
                        Button::new("chart-save")
                            .small()
                            .primary()
                            .label(yss_ui::text::t("common.save"))
                            .disabled(self.busy() || !self.dirty())
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
    }
}
impl ChartEditor {
    fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.preview_loading || self.file.reading() {
            return appearance::empty_state(
                IconName::ChartLine,
                yss_ui::text::t("native.charts.loading"),
                yss_ui::text::t("native.charts.preparingData"),
                cx,
            )
            .into_any_element();
        }
        let Some(preview) = &self.preview else {
            return appearance::empty_state(
                IconName::ChartLine,
                yss_ui::text::t("native.charts.previewUnavailable"),
                yss_ui::text::t("native.charts.refreshHint"),
                cx,
            )
            .into_any_element();
        };
        match preview.as_ref() {
            PreviewData::Empty(message) => appearance::empty_state(
                IconName::ChartLine,
                yss_ui::text::translate("chart.previewEmpty"),
                yss_ui::text::translate(message),
                cx,
            )
            .into_any_element(),
            PreviewData::Failed(_) => div().into_any_element(),
            PreviewData::Histogram {
                bins,
                column,
                other_count,
            } => {
                if bins.is_empty() {
                    return appearance::empty_state(
                        IconName::ChartLine,
                        yss_ui::text::t("native.charts.noDistribution"),
                        yss_ui::text::t("native.charts.chooseValidColumn"),
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
                            .child(yss_ui::text::format(
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
                                .child(yss_ui::text::plural(
                                    "native.charts.otherValues",
                                    *other_count,
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
                        .child(yss_ui::text::format(
                            "native.charts.plotSummary",
                            &[
                                (
                                    "x",
                                    data.result.x_label.as_deref().unwrap_or("X").to_string(),
                                ),
                                (
                                    "y",
                                    data.result.y_label.as_deref().unwrap_or("Y").to_string(),
                                ),
                                (
                                    "points",
                                    yss_ui::text::plural(
                                        "counts.plotPoints",
                                        data.result.data.len(),
                                    ),
                                ),
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
