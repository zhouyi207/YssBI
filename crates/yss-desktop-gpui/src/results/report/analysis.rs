//! Selected analyses read the existing result once and keep local sections under its lease.
mod query;
mod statistics;

use super::{
    display,
    section::{Section, Source, Title},
};
use crate::{
    plots::{
        cartesian::{CartesianData, CartesianPlot},
        correlogram::{Correlogram, CorrelogramData},
    },
    services::NativeServices,
};
use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
};
use query::AnalysisData;
pub(super) use query::series;
use std::sync::Arc;
use yss_application::graph::results::report::ResultAnalysisRequest;
use yss_graph_execution::result::ResultReference;

#[derive(Clone, Copy)]
pub(super) enum AnalysisKind {
    Diagnostics,
    AcfPacf,
    SerialTests,
    Hypothesis,
}

impl AnalysisKind {
    fn request(self) -> ResultAnalysisRequest {
        match self {
            Self::Diagnostics => ResultAnalysisRequest::Diagnostics,
            Self::AcfPacf => ResultAnalysisRequest::AcfPacf,
            Self::SerialTests => ResultAnalysisRequest::SerialTests,
            Self::Hypothesis => ResultAnalysisRequest::Hypothesis,
        }
    }
}

pub(super) struct AnalysisView {
    services: Arc<NativeServices>,
    reference: ResultReference,
    kind: AnalysisKind,
    data: Option<AnalysisData>,
    diagnostics: Vec<Entity<Section>>,
    loading: bool,
    error: bool,
    task: Option<gpui::Task<()>>,
}

impl AnalysisView {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        kind: AnalysisKind,
    ) -> Self {
        Self {
            services,
            reference,
            kind,
            data: None,
            diagnostics: vec![],
            loading: false,
            error: false,
            task: None,
        }
    }
    pub fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = false;
        let reference = self.reference;
        let kind = self.kind;
        let task = self.services.run(move |services| {
            query::prepare(
                services
                    .application
                    .analyze_result(reference, kind.request())?,
            )
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, _, cx| {
                view.loading = false;
                match result {
                    Ok(mut data) => {
                        if let AnalysisData::Diagnostics { tests, .. } = &mut data {
                            view.diagnostics = std::mem::take(tests)
                                .into_iter()
                                .map(|(name, value)| {
                                    let (title, value) = match value {
                                        Ok(value) => (Title::Content(name), value),
                                        Err(reason) => (
                                            Title::Unavailable(name),
                                            Arc::new(yss_node_kernel::RuntimeValue::Scalar(
                                                yss_data_contract::TabularScalar::String(
                                                    reason.into_boxed_str(),
                                                ),
                                            )),
                                        ),
                                    };
                                    cx.new(|_| {
                                        Section::new(
                                            view.services.clone(),
                                            view.reference,
                                            title,
                                            Source::Value(value),
                                            false,
                                        )
                                    })
                                })
                                .collect();
                        }
                        view.data = Some(data);
                    }
                    Err(_) => view.error = true,
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}

impl Render for AnalysisView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div().w_full().min_w_0().flex().flex_col().gap_3();
        if let Some(data) = &self.data {
            body = body.child(match data {
                AnalysisData::Hypothesis(data) => {
                    statistics::hypothesis(data, cx).into_any_element()
                }
                AnalysisData::Serial(data) => statistics::serial(data, cx).into_any_element(),
                AnalysisData::Acf {
                    acf,
                    pacf,
                    observations,
                } => div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(crate::text::format(
                        "native.reports.observationsCount",
                        &[("count", observations.to_string())],
                    ))
                    .children([(acf, false, "ACF"), (pacf, true, "PACF")].map(
                        |(data, partial, label)| {
                            div()
                                .h(px(260.))
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(label)
                                .child(div().flex_1().min_h_0().child(Correlogram {
                                    data: data.clone(),
                                    partial,
                                    id: format!("analysis-{label}-{}", cx.entity_id()).into(),
                                }))
                        },
                    ))
                    .into_any_element(),
                AnalysisData::Diagnostics {
                    density, reason, ..
                } => div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(self.diagnostics.iter().cloned())
                    .child(display::section(
                        "native.reports.leverageDensity",
                        div()
                            .when_some(reason.as_ref(), |body, reason| {
                                body.child(crate::text::format(
                                    "native.reports.unavailableReason",
                                    &[("reason", reason.clone())],
                                ))
                            })
                            .when_some(density.as_ref(), |body, data| {
                                body.child(axes(
                                    "native.reports.leverage",
                                    "native.reports.density",
                                ))
                                .child(div().h(px(280.)).child(CartesianPlot {
                                    data: data.clone(),
                                    id: format!("leverage-density-{}", cx.entity_id()).into(),
                                    generation: 0,
                                    show_points: false,
                                }))
                            }),
                        cx,
                    ))
                    .into_any_element(),
            });
        }
        body.when(self.loading, |body| {
            body.child(crate::text::translate("common.loading"))
        })
        .when(self.error, |body| {
            body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(cx.theme().danger)
                            .child(crate::text::translate("native.reports.analysisFailed")),
                    )
                    .child(
                        Button::new("retry-analysis")
                            .small()
                            .ghost()
                            .label(crate::text::translate("common.retry"))
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, window, cx| view.load(window, cx))),
                    ),
            )
        })
    }
}

pub(super) fn axes(x: &str, y: &str) -> impl IntoElement + use<> {
    div().text_xs().child(crate::text::format(
        "native.plots.axes",
        &[
            ("x", crate::text::translate(x)),
            ("y", crate::text::translate(y)),
        ],
    ))
}
