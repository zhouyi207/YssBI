use super::super::display::number;
use gpui::{App, ClipboardItem, Div, div, prelude::*};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit_assets::IconName;
use yss_application::graph::results::report::{HypothesisTestOutput, SerialTestsOutput};

fn card(label: &str, statistic: String, probability: Option<f64>, cx: &App) -> Div {
    div()
        .min_w(gpui::px(190.))
        .flex_1()
        .p_3()
        .rounded_md()
        .bg(cx.theme().muted)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::translate(label)),
        )
        .child(
            div()
                .text_sm()
                .font_family(cx.theme().mono_font_family.clone())
                .child(statistic),
        )
        .when_some(probability, |body, p| {
            body.child(
                div()
                    .text_xs()
                    .text_color(if p < 0.05 {
                        cx.theme().green
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(format!(
                        "p = {}{}",
                        number(p, 4),
                        if p < 0.05 { " *" } else { "" }
                    )),
            )
        })
}

pub(super) fn serial(value: &SerialTestsOutput, cx: &App) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_3()
        .when_some(value.bg.as_ref(), |body, bg| {
            body.child(card(
                "native.reports.bg",
                format!("χ²({}) = {}", bg.lags, number(bg.stat, 4)),
                Some(bg.p_value),
                cx,
            ))
        })
        .when_some(value.q.as_ref(), |body, q| {
            body.child(card(
                "native.reports.ljungBox",
                format!("Q({}) = {}", q.lags, number(q.stat, 4)),
                Some(q.p_value),
                cx,
            ))
        })
        .child(card(
            "native.reports.dw",
            format!("DW = {}", number(value.dw.d, 4)),
            None,
            cx,
        ))
}

pub(super) fn hypothesis(value: &HypothesisTestOutput, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div().flex().flex_wrap().gap_3().children(
                [
                    ("native.reports.nullHypothesis", &value.h0_form),
                    ("native.reports.alternativeHypothesis", &value.h1_form),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (key, form))| {
                    let copied = form.clone();
                    div()
                        .min_w(gpui::px(220.))
                        .flex_1()
                        .p_3()
                        .rounded_md()
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(div().text_xs().child(crate::text::translate(key)))
                                .child(
                                    Button::new(("copy-hypothesis", index))
                                        .small()
                                        .ghost()
                                        .icon(IconName::Copy)
                                        .tooltip(crate::text::translate("menubar.copy"))
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copied.clone(),
                                            ))
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_family(cx.theme().mono_font_family.clone())
                                .whitespace_normal()
                                .child(form.clone()),
                        )
                }),
            ),
        )
        .child(card(
            if value.test_type == "t" {
                "native.reports.tStatistic"
            } else {
                "native.reports.metrics.fStatistic"
            },
            number(value.stat, 4),
            Some(value.p_value),
            cx,
        ))
        .child(div().text_xs().child(crate::text::format(
            "native.reports.degreesOfFreedom",
            &[
                ("first", value.df1.to_string()),
                ("second", value.df2.to_string()),
            ],
        )))
}
