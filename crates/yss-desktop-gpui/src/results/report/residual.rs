//! Residual controls submit bounded reads; the Application owns filtering and leverage ranks.
mod controls;
mod render;

use crate::{
    plots::cartesian::{CartesianData, CartesianKind, CartesianOptions, ScatterObservation},
    services::NativeServices,
};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext, Context, Entity, Subscription, Window};
use std::sync::Arc;
use yss_application::{
    chart::PlotPoint,
    graph::results::report::{
        ResidualPlotProjection, ResultAnalysisProjection, ResultAnalysisRequest,
    },
};
use yss_graph_execution::result::ResultReference;

#[derive(Clone, Copy, Default, PartialEq)]
struct Selection {
    adjacent: bool,
    range: Option<[f64; 2]>,
    highlight: f64,
}

struct ResidualData {
    plot: Arc<CartesianData>,
    selection: Selection,
    displayed: usize,
    matched: usize,
    total: usize,
    sampled: bool,
    highlight_available: bool,
}

impl ResidualData {
    fn new(value: ResidualPlotProjection, selection: Selection) -> anyhow::Result<Self> {
        anyhow::ensure!(
            value
                .points
                .iter()
                .all(|p| p.x.is_finite() && p.y.is_finite()),
            "invalid residual coordinates"
        );
        let observations = value
            .points
            .iter()
            .map(|p| ScatterObservation {
                number: p.observation,
                highlighted: p.highlighted,
            })
            .collect();
        let displayed = value.points.len();
        let plot = Arc::new(CartesianData::new(
            super::analysis::series(
                value
                    .points
                    .into_iter()
                    .map(|p| PlotPoint { x: p.x, y: p.y })
                    .collect(),
            ),
            CartesianOptions {
                observations: Some(observations),
                zero_line: true,
                symmetric_y: true,
                ..CartesianOptions::new(CartesianKind::Scatter)
            },
        ));
        Ok(Self {
            plot,
            selection,
            displayed,
            matched: value.matched_count,
            total: value.total_count,
            sampled: value.sampled,
            highlight_available: value.highlight_available,
        })
    }
}

pub(super) struct ResidualView {
    services: Arc<NativeServices>,
    reference: ResultReference,
    selection: Selection,
    minimum: Entity<InputState>,
    maximum: Entity<InputState>,
    highlight: Entity<InputState>,
    data: Option<ResidualData>,
    loading: bool,
    error: bool,
    generation: u64,
    task: Option<gpui_kit::Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ResidualView {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let minimum = cx.new(|cx| InputState::new(window, cx));
        let maximum = cx.new(|cx| InputState::new(window, cx));
        let highlight = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        let subscriptions = [&minimum, &maximum, &highlight]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |_, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            services,
            reference,
            selection: Selection::default(),
            minimum,
            maximum,
            highlight,
            data: None,
            loading: false,
            error: false,
            generation: 0,
            task: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if !self.error
            && self
                .data
                .as_ref()
                .is_some_and(|data| data.selection == self.selection)
        {
            cx.notify();
            return;
        }
        self.loading = true;
        self.error = false;
        self.generation += 1;
        let reference = self.reference;
        let selection = self.selection;
        let task = self.services.run(move |services| {
            let ResultAnalysisProjection::ResidualPlot(value) =
                services.application.analyze_result(
                    reference,
                    ResultAnalysisRequest::ResidualPlot {
                        max_points: 2000,
                        x_range: selection.range,
                        adjacent: selection.adjacent,
                        highlight_top_percent: (selection.highlight > 0.)
                            .then_some(selection.highlight),
                    },
                )?
            else {
                anyhow::bail!("unexpected residual result")
            };
            ResidualData::new(value, selection)
        });
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, _, cx| {
                view.loading = false;
                match result {
                    Ok(data) => view.data = Some(data),
                    Err(_) => view.error = true,
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn reset_range(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.range = None;
        self.minimum
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.maximum
            .update(cx, |input, cx| input.set_value("", window, cx));
    }
}
