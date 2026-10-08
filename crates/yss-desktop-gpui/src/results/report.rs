//! Report entities retain local presentation state under the result panel's lease.
mod page;
mod section;

use std::sync::Arc;

use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*};
use gpui_component::ActiveTheme;
use yss_application::graph::results::report::structured::ReportSection;
use yss_data_contract::TabularScalar;
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use crate::services::NativeServices;
use section::{Section, Source, Title};

pub struct ReportView {
    sections: Vec<Entity<Section>>,
    invalid: bool,
}

impl ReportView {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        value: Arc<RuntimeValue>,
        declarations: Result<Vec<ReportSection>, ()>,
        cx: &mut Context<Self>,
    ) -> Self {
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
                Source::Value(value, 0),
                open,
            )
        }));
        Self { sections, invalid }
    }
}

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
                    .children(self.sections.iter().cloned()),
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
