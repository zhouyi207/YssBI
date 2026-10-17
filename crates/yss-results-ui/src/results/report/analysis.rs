//! Selected analyses read the existing result once and keep local sections under its lease.
mod query;
mod statistics;

use super::{
    display,
    section::{Section, Source, Title},
};
use crate::plots::{
    cartesian::{CartesianData, CartesianPlot},
    correlogram::{Correlogram, CorrelogramData},
};
use gpui_kit::{Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use query::AnalysisData;
pub(super) use query::series;
use std::sync::Arc;
use yss_application::graph::results::report::ResultAnalysisRequest;
use yss_desktop_services::ApplicationTasks;
use yss_graph_execution::result::ResultReference;
use yss_ui::lifecycle::LogReleased;

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
    services: Arc<ApplicationTasks>,
    reference: ResultReference,
    kind: AnalysisKind,
    data: Option<AnalysisData>,
    diagnostics: Vec<Entity<Section>>,
    loading: bool,
    error: bool,
    task: Option<gpui_kit::Task<()>>,
}

impl AnalysisView {
    pub fn new(
        services: Arc<ApplicationTasks>,
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
            query::prepare(services.analyze_result(reference, kind.request())?)
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update_in(cx, |view, window, cx| {
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
                    Err(_) => {
                        view.error = true;
                        yss_ui::notifications::Notifications::new(window).error_with_retry(
                            yss_ui::text::t("native.reports.analysisFailed"),
                            cx,
                            |view, window, cx| view.load(window, cx),
                        );
                    }
                }
                cx.notify();
            })
            .log_released();
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
                    .child(yss_ui::text::format(
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
                                body.child(yss_ui::text::format(
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
            body.child(yss_ui::text::translate("common.loading"))
        })
    }
}

pub(super) fn axes(x: &str, y: &str) -> impl IntoElement + use<> {
    div().text_xs().child(yss_ui::text::format(
        "native.plots.axes",
        &[
            ("x", yss_ui::text::translate(x)),
            ("y", yss_ui::text::translate(y)),
        ],
    ))
}
