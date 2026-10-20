//! Read-only node diagnostics from the current editor projection.
use super::GraphDetails;
use crate::feedback::{diagnostic_description, graph_diagnostic};
use crate::problems::location;
use gpui_kit::component::{Sizable, button::Button};
use gpui_kit::{AnyElement, ClipboardItem, Context, IntoElement, div, prelude::*};
use yss_graph_editor::projection::{EditorDiagnosticSeverity, EditorNodeModel};
use yss_ui::inspector::{self, InlineMessage, InspectorSection, Pager, Severity};
use yss_ui::text::{format, plural, translate};

pub(super) const PAGE_DIAGNOSTICS: usize = 50;

impl GraphDetails {
    pub(super) fn render_diagnostic_summary(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if node.diagnostics.is_empty() {
            return div().into_any_element();
        }
        let mut errors = 0;
        let mut warnings = 0;
        let mut information = 0;
        let mut blocking = 0;
        for diagnostic in &node.diagnostics {
            match diagnostic.severity {
                EditorDiagnosticSeverity::Error => errors += 1,
                EditorDiagnosticSeverity::Warning => warnings += 1,
                EditorDiagnosticSeverity::Information => information += 1,
            }
            blocking += usize::from(diagnostic.blocking);
        }
        let severity = if errors > 0 {
            Severity::Error
        } else if warnings > 0 {
            Severity::Warning
        } else {
            Severity::Information
        };
        let action = if node
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_ref() == "graph.input.unbound")
        {
            "detail.diagnosticSummary.connectInputs"
        } else if node.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.location,
                yss_graph_analysis_contract::DiagnosticLocation::Parameter { .. }
            )
        }) {
            "detail.diagnosticSummary.configureParameters"
        } else {
            "detail.diagnosticSummary.review"
        };
        inspector::status_band(
            InlineMessage::new(severity, translate(action)).subject(format(
                "detail.diagnosticSummary.counts",
                &[
                    ("errors", plural("counts.errors", errors)),
                    ("warnings", plural("counts.warnings", warnings)),
                    ("notes", plural("counts.notes", information)),
                    ("blocking", plural("counts.blocking", blocking)),
                ],
            )),
            cx,
        )
        .into_any_element()
    }

    pub(super) fn render_diagnostics(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(projection) = self
            .projection
            .as_ref()
            .filter(|_| !node.diagnostics.is_empty())
        else {
            return div().into_any_element();
        };
        let epoch = self.epoch;
        let pages = node.diagnostics.len().div_ceil(PAGE_DIAGNOSTICS);
        let rows = node
            .diagnostics
            .iter()
            .skip(self.diagnostics_page * PAGE_DIAGNOSTICS)
            .take(if self.diagnostics_open {
                PAGE_DIAGNOSTICS
            } else {
                0
            })
            .map(|diagnostic| {
                let severity = match diagnostic.severity {
                    EditorDiagnosticSeverity::Error => Severity::Error,
                    EditorDiagnosticSeverity::Warning => Severity::Warning,
                    EditorDiagnosticSeverity::Information => Severity::Information,
                };
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        InlineMessage::new(
                            severity,
                            diagnostic_description(diagnostic, projection, Some(node)),
                        )
                        .when_some(
                            location::label(&diagnostic.location, projection, Some(node))
                                .filter(|label| label != location::node_title(node)),
                            |message, label| message.subject(label),
                        ),
                    )
                    .when(diagnostic.blocking, |row| {
                        row.child(inspector::hint(translate("panel.problemsBlocking"), cx))
                    })
            });
        InspectorSection::new("node-diagnostics", translate("detail.sections.diagnostics"))
            .count(node.diagnostics.len())
            .collapsible(
                self.diagnostics_open,
                cx.listener(move |view, open: &bool, _, cx| {
                    if view.epoch == epoch {
                        view.diagnostics_open = *open;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("node-diagnostic-rows")
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(pages > 1, |rows| {
                        rows.max_h_80()
                            .overflow_y_scroll()
                            .track_scroll(&self.diagnostics_scroll)
                    })
                    .children(rows),
            )
            .when(pages > 1, |section| {
                section.child(div().flex().justify_end().child(
                    Pager::new("node-diagnostics-pager", self.diagnostics_page, pages).on_change(
                        cx.listener(move |view, page: &usize, _, cx| {
                            if view.epoch == epoch {
                                view.diagnostics_page = *page;
                                view.diagnostics_scroll
                                    .set_offset(gpui_kit::Point::default());
                                cx.notify();
                            }
                        }),
                    ),
                ))
            })
            .into_any_element()
    }

    pub(super) fn render_technical_details(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let epoch = self.epoch;
        InspectorSection::new(
            "node-technical",
            translate("detail.diagnosticSummary.technical"),
        )
        .collapsible(
            self.technical_open,
            cx.listener(move |view, open: &bool, _, cx| {
                if view.epoch == epoch {
                    view.technical_open = *open;
                    cx.notify();
                }
            }),
        )
        .when(self.technical_open, |section| {
            section
                .child(inspector::property_list([
                    inspector::property_row(
                        translate("detail.diagnosticSummary.nodeId"),
                        node.node_id.to_string(),
                        cx,
                    ),
                    inspector::property_row(
                        translate("detail.diagnosticSummary.nodeType"),
                        node.node_type.as_str().to_owned(),
                        cx,
                    ),
                ]))
                .child(
                    Button::new("copy-node-technical")
                        .small()
                        .outline()
                        .label(translate("detail.diagnosticSummary.copy"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.epoch != epoch {
                                return;
                            }
                            if let Some(node) = view.node() {
                                let mut report = format!(
                                    "Node: {}\nType: {}\n\n",
                                    node.node_id,
                                    node.node_type.as_str(),
                                );
                                for port in &node.ports {
                                    report.push_str(&format!(
                                        "{}: {}\n",
                                        port.display
                                            .instance_label
                                            .as_deref()
                                            .unwrap_or(&port.display.label),
                                        port.address,
                                    ));
                                }
                                for diagnostic in &node.diagnostics {
                                    report.push_str(&format!(
                                        "\n{}\n{diagnostic:#?}\n",
                                        graph_diagnostic(diagnostic),
                                    ));
                                }
                                cx.write_to_clipboard(ClipboardItem::new_string(report));
                            }
                        })),
                )
                .child(inspector::hint(
                    translate("detail.diagnosticSummary.copyHint"),
                    cx,
                ))
        })
        .into_any_element()
    }
}
