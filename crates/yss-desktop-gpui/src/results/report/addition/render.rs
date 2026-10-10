use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    collapsible::Collapsible,
    input::Input,
};
use gpui_kit::{Div, div, prelude::*, px};

impl AdditionForm {
    pub fn render(&self, cx: &mut Context<ReportView>) -> Div {
        let available = LinearSummaryContent::ALL
            .into_iter()
            .filter(|content| !content.included(&self.options))
            .collect::<Vec<_>>();
        let body = div()
            .flex()
            .flex_col()
            .gap_3()
            .pt_3()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::translate("reportSummary.help")),
            )
            .when(available.is_empty(), |body| {
                body.child(crate::text::translate("reportSummary.allIncluded"))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(available.iter().enumerate().map(|(index, content)| {
                        let content = *content;
                        div().min_w(px(220.)).child(
                            Checkbox::new(("report-addition", index))
                                .label(crate::text::translate(label(content)))
                                .checked(self.selected.contains(&content))
                                .disabled(self.busy)
                                .on_click(cx.listener(move |view, checked: &bool, _, cx| {
                                    if let Some(form) = &mut view.addition
                                        && !form.busy
                                    {
                                        if *checked {
                                            form.selected.insert(content);
                                        } else {
                                            form.selected.remove(&content);
                                        }
                                        cx.notify();
                                    }
                                })),
                        )
                    })),
            )
            .when(
                self.selected.contains(&LinearSummaryContent::AcfPacf),
                |body| body.child(field("reportSummary.acfLag", &self.acf_lag, self.busy)),
            )
            .when(
                self.selected.contains(&LinearSummaryContent::SerialTests),
                |body| {
                    body.child(field(
                        "reportSummary.serialLag",
                        &self.serial_lag,
                        self.busy,
                    ))
                    .child(
                        Checkbox::new("report-bg-nomiss")
                            .label(crate::text::translate("reportSummary.nomiss0"))
                            .checked(self.nomiss0)
                            .disabled(self.busy)
                            .on_click(cx.listener(|view, value: &bool, _, cx| {
                                if let Some(form) = &mut view.addition
                                    && !form.busy
                                {
                                    form.nomiss0 = *value;
                                    cx.notify();
                                }
                            })),
                    )
                },
            )
            .when(
                self.selected
                    .contains(&LinearSummaryContent::HypothesisTest),
                |body| {
                    body.child(
                        div()
                            .text_sm()
                            .child(crate::text::translate("reportSummary.hypothesis")),
                    )
                    .child(Input::new(&self.hypothesis).small().disabled(self.busy))
                    .child(
                        div()
                            .id("report-parameter-names")
                            .max_h(px(120.))
                            .overflow_y_scroll()
                            .text_xs()
                            .whitespace_normal()
                            .child(crate::text::format(
                                "reportSummary.paramNames",
                                &[("names", self.param_names.clone())],
                            )),
                    )
                },
            )
            .when(!available.is_empty(), |body| {
                body.child(
                    Button::new("submit-report-addition")
                        .small()
                        .label(crate::text::translate(if self.busy {
                            "reportSummary.computing"
                        } else {
                            "reportSummary.compute"
                        }))
                        .disabled(self.busy || !self.request(cx).is_valid())
                        .on_click(cx.listener(|view, _, _, cx| {
                            if let Some(form) = &view.addition
                                && !form.busy
                            {
                                let request = form.request(cx);
                                if request.is_valid() {
                                    cx.emit(request);
                                }
                            }
                        })),
                )
            })
            .when(self.busy, |body| {
                body.child(
                    div()
                        .text_sm()
                        .child(crate::text::translate("reportSummary.pending")),
                )
            })
            .when_some(self.error, |body, error| {
                body.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(crate::text::translate(error)),
                )
            });
        div()
            .w_full()
            .min_w_0()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .p_3()
            .child(
                Collapsible::new()
                    .open(self.open)
                    .child(
                        Button::new("report-additions-toggle")
                            .small()
                            .ghost()
                            .w_full()
                            .icon(if self.open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .label(crate::text::translate("reportSummary.add"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                if let Some(form) = &mut view.addition {
                                    form.open = !form.open;
                                    cx.notify();
                                }
                            })),
                    )
                    .content(body),
            )
    }
}

fn field(key: &str, state: &Entity<InputState>, busy: bool) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(div().text_sm().child(crate::text::translate(key)))
        .child(
            div()
                .w(px(110.))
                .child(Input::new(state).small().disabled(busy)),
        )
        .child(div().text_xs().child("1–40"))
}
