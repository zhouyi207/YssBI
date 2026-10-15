use super::*;
use crate::plots::cartesian::CartesianPlot;
use gpui_kit::{IntoElement, Render, div, prelude::*, px};

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
                body.child(div().text_xs().child(yss_ui::text::format(
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
                            .child(yss_ui::text::translate("native.reports.noLeverage")),
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
                    body.child(yss_ui::text::translate(
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
                body.child(yss_ui::text::translate("common.loading"))
            })
    }
}
