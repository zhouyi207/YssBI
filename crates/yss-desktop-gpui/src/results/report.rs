//! Report entities retain local presentation state under the result panel's lease.
mod addition;
mod analysis;
mod display;
mod linear;
mod page;
mod residual;
mod section;

use std::sync::Arc;

use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Window, div, prelude::*};
use gpui_component::ActiveTheme;
use yss_data_contract::TabularScalar;
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use crate::services::NativeServices;
use section::{Section, Source, Title};
use yss_application::graph::results::report::addition::LinearSummaryAddition;

pub struct ReportView {
    sections: Vec<Entity<Section>>,
    invalid: bool,
    linear: Option<Entity<linear::LinearReport>>,
    addition: Option<addition::AdditionForm>,
}

impl ReportView {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        value: Arc<RuntimeValue>,
        report: super::query::ReportContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let declarations = match report {
            super::query::ReportContent::Structured(declarations) => declarations,
            super::query::ReportContent::Linear(report) => {
                let addition = Some(addition::AdditionForm::new(
                    &report.summary,
                    &report.param_names,
                    window,
                    cx,
                ));
                return Self {
                    sections: vec![],
                    invalid: false,
                    linear: Some(cx.new(|cx| linear::LinearReport::new(services, *report, cx))),
                    addition,
                };
            }
        };
        let invalid = declarations.is_err();
        let mut sections = declarations
            .unwrap_or_default()
            .into_iter()
            .map(|section| {
                cx.new(|_| {
                    Section::new(
                        services.clone(),
                        reference,
                        Title::Content(section.title),
                        Source::from(section.content),
                        false,
                    )
                })
            })
            .collect::<Vec<_>>();
        let open = sections.is_empty();
        sections.push(cx.new(|_| {
            Section::new(
                services,
                reference,
                Title::Key("reportSections.structuredResult"),
                Source::Value(value),
                open,
            )
        }));
        Self {
            sections,
            invalid,
            linear: None,
            addition: None,
        }
    }

    pub fn set_addition_state(
        &mut self,
        busy: bool,
        error: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = &mut self.addition {
            form.busy = busy;
            form.error = error;
            form.open |= busy || error.is_some();
            cx.notify();
        }
    }
}

impl EventEmitter<LinearSummaryAddition> for ReportView {}

impl Render for ReportView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("structured-report")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .w_full()
                    .max_w(gpui::px(1100.))
                    .mx_auto()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .when(self.invalid, |body| {
                        body.child(
                            div()
                                .text_color(cx.theme().danger)
                                .child(crate::text::translate("native.reports.invalidDisplay")),
                        )
                    })
                    .when_some(self.addition.as_ref(), |body, form| {
                        body.child(form.render(cx))
                    })
                    .children(self.sections.iter().cloned())
                    .children(self.linear.iter().cloned()),
            )
    }
}

/// Report formatting preserves exact integers and text; null remains distinct from empty text.
pub(super) fn scalar(value: &RuntimeValue) -> String {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::String(text)) => text.to_string(),
        RuntimeValue::Scalar(TabularScalar::Null) => "—".into(),
        _ => super::value::display(value),
    }
}
