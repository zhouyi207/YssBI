//! Unsubmitted report choices; the parent result panel dispatches the Application use case.
mod render;
use super::ReportView;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext, Context, Entity, Subscription, Window};
use std::collections::BTreeSet;
use yss_application::graph::results::report::{
    LinearSummaryOptions,
    addition::{LinearSummaryAddition, LinearSummaryContent},
};

pub(super) struct AdditionForm {
    options: LinearSummaryOptions,
    param_names: String,
    selected: BTreeSet<LinearSummaryContent>,
    acf_lag: Entity<InputState>,
    serial_lag: Entity<InputState>,
    hypothesis: Entity<InputState>,
    nomiss0: bool,
    pub open: bool,
    pub busy: bool,
    pub error: Option<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl AdditionForm {
    pub fn new(
        options: &LinearSummaryOptions,
        param_names: &[String],
        window: &mut Window,
        cx: &mut Context<ReportView>,
    ) -> Self {
        let options = options.clone();
        let acf_lag =
            cx.new(|cx| InputState::new(window, cx).default_value(options.acf_max_lag.to_string()));
        let serial_lag =
            cx.new(|cx| InputState::new(window, cx).default_value(options.serial_lags.to_string()));
        let hypothesis =
            cx.new(|cx| InputState::new(window, cx).default_value(options.hypothesis.clone()));
        let subscriptions = [&acf_lag, &serial_lag, &hypothesis]
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
            nomiss0: options.bg_nomiss0,
            options,
            param_names: param_names.join(", "),
            selected: BTreeSet::new(),
            acf_lag,
            serial_lag,
            hypothesis,
            open: false,
            busy: false,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    fn request(&self, cx: &gpui_kit::App) -> LinearSummaryAddition {
        LinearSummaryAddition {
            contents: self.selected.clone(),
            acf_max_lag: self.acf_lag.read(cx).value().trim().parse().unwrap_or(0),
            serial_lags: self.serial_lag.read(cx).value().trim().parse().unwrap_or(0),
            bg_nomiss0: self.nomiss0,
            hypothesis: self.hypothesis.read(cx).value().to_string(),
        }
    }
}

fn label(content: LinearSummaryContent) -> &'static str {
    use LinearSummaryContent::*;
    match content {
        ModelSummary => "reportSections.modelSummary",
        CoefficientTable => "reportSections.coefficientTable",
        CoefficientChart => "reportSections.coefficientMagnitude",
        Equation => "reportSections.equation",
        Anova => "reportSections.anova",
        Diagnostics => "reportSections.diagnosticTests",
        ResidualPlot => "reportSections.residualPlot",
        Observations => "reportSections.observations",
        AcfPacf => "reportSections.acfPacf",
        SerialTests => "reportSections.serialTests",
        HypothesisTest => "reportSections.hypothesisTest",
    }
}
