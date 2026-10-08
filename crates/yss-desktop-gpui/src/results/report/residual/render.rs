use super::*;
use crate::plots::cartesian::CartesianPlot;
use gpui::{IntoElement, Render, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
};

impl Render for ResidualView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.controls(cx))
            .when_some(self.data.as_ref(), |body, data| {
                body.child(div().text_xs().child(crate::text::format(
                    if data.sampled {
                        "native.reports.sampledResiduals"
                    } else {
                        "native.reports.residualCount"
                    },
                    &[
                        ("shown", data.displayed.to_string()),
                        ("matched", data.matched.to_string()),
                        ("total", data.total.to_string()),
                    ],
                )))
                .when(!data.highlight_available, |body| {
                    body.child(
                        div()
                            .text_xs()
                            .child(crate::text::translate("native.reports.noLeverage")),
                    )
                })
                .child(super::super::analysis::axes(
                    if data.selection.adjacent {
                        "native.reports.previousResidual"
                    } else {
                        "native.reports.fitted"
                    },
                    "native.reports.residual",
                ))
                .when(data.displayed == 0, |body| {
                    body.child(crate::text::translate(
                        "native.reports.noMatchingObservations",
                    ))
                })
                .when(data.displayed > 0, |body| {
                    body.child(div().h(px(350.)).child(CartesianPlot {
                        data: data.plot.clone(),
                        id: format!("report-residuals-{}", cx.entity_id()).into(),
                        generation: self.generation,
                        show_points: false,
                    }))
                })
            })
            .when(self.loading, |body| {
                body.child(crate::text::translate("common.loading"))
            })
            .when(self.error, |body| {
                body.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_color(cx.theme().danger)
                                .child(crate::text::translate("native.reports.analysisFailed")),
                        )
                        .child(
                            Button::new("retry-residuals")
                                .small()
                                .ghost()
                                .disabled(self.loading)
                                .label(crate::text::translate("common.retry"))
                                .on_click(cx.listener(|view, _, window, cx| view.load(window, cx))),
                        ),
                )
            })
    }
}
