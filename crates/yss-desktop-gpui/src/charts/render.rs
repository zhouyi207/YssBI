use super::{
    ChartEditor, SaveChart,
    details::chart_type_label,
    plot::CartesianPlot,
    query::{HistogramDatum, PreviewData},
};
use crate::{appearance, assets::NativeIcon};
use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    chart::BarChart,
};

#[derive(Clone, PartialEq, Eq, Hash)]
struct HistogramBand {
    index: usize,
    label: gpui::SharedString,
}
impl From<HistogramBand> for gpui::SharedString {
    fn from(value: HistogramBand) -> Self {
        value.label
    }
}

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
                    .child(Icon::new(NativeIcon::Chart).size_4())
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
                            .label("刷新")
                            .disabled(self.busy())
                            .on_click(cx.listener(|view, _, window, cx| view.refresh(window, cx))),
                    )
                    .child(
                        Button::new("chart-save")
                            .small()
                            .primary()
                            .label("保存")
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
    fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.preview_loading || self.reading {
            return appearance::empty_state(
                NativeIcon::Chart,
                "正在读取图表",
                "数据准备在后台执行",
                cx,
            )
            .into_any_element();
        }
        let Some(preview) = &self.preview else {
            return appearance::empty_state(
                NativeIcon::Chart,
                "预览暂不可用",
                "检查属性设置后刷新图表",
                cx,
            )
            .into_any_element();
        };
        match preview.as_ref() {
            PreviewData::Empty(message) => {
                appearance::empty_state(NativeIcon::Chart, "配置图表", message.to_string(), cx)
                    .into_any_element()
            }
            PreviewData::Failed(message) => {
                appearance::empty_state(NativeIcon::Chart, "预览未读取", message.to_string(), cx)
                    .into_any_element()
            }
            PreviewData::Histogram {
                bins,
                column,
                other_count,
            } => {
                if bins.is_empty() {
                    return appearance::empty_state(
                        NativeIcon::Chart,
                        "暂无分布数据",
                        "选择有有效值的列后重试",
                        cx,
                    )
                    .into_any_element();
                }
                let color = cx.theme().primary;
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .p_4()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{column} · 频数")),
                    )
                    .child(
                        div().flex_1().min_h_0().child(
                            BarChart::new(bins.clone())
                                .id(format!("chart-bars-{}", cx.entity_id()))
                                .band(|datum: &HistogramDatum| HistogramBand {
                                    index: datum.index,
                                    label: datum.label.clone().into(),
                                })
                                .value(|datum: &HistogramDatum| datum.count as f64)
                                .fill(move |_, _, _, _| color)
                                .value_axis(true)
                                .band_tick_count(8)
                                .appear(false),
                        ),
                    )
                    .when(*other_count > 0, |view| {
                        view.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("另有 {other_count} 个值归入其他类别")),
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
                        .child(format!(
                            "X：{} · Y：{} · {} 个绘图点",
                            data.result.x_label.as_deref().unwrap_or("X"),
                            data.result.y_label.as_deref().unwrap_or("Y"),
                            data.result.data.len()
                        )),
                )
                .child(div().flex_1().min_h_0().child(CartesianPlot {
                    data: data.clone(),
                    id: format!("chart-points-{}", cx.entity_id()).into(),
                    generation: self.preview_generation,
                }))
                .child(div().h(px(8.)))
                .into_any_element(),
        }
    }
}
