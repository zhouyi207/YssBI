//! Read-only node diagnostics from the current editor projection.
use super::DetailsPanel;
use crate::text::{graph_diagnostic, translate};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
};
use gpui_kit_assets::IconName;
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_document::{NodeId, PortAddress};
use yss_graph_editor::projection::{
    EditorDiagnosticModel, EditorDiagnosticSeverity, EditorNodeModel, EditorProjectionModel,
};

pub(super) const PAGE_DIAGNOSTICS: usize = 50;

impl DetailsPanel {
    pub(super) fn render_diagnostics(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if node.diagnostics.is_empty() {
            return div().into_any_element();
        }
        let epoch = self.epoch;
        let pages = node.diagnostics.len().div_ceil(PAGE_DIAGNOSTICS);
        Collapsible::new()
            .w_full()
            .min_w_0()
            .open(self.diagnostics_open)
            .child(
                Button::new("node-diagnostics-toggle")
                    .small()
                    .ghost()
                    .w_full()
                    .h_7()
                    .px_2()
                    .rounded_none()
                    .justify_start()
                    .bg(cx.theme().muted.opacity(0.6))
                    .text_xs()
                    .icon(if self.diagnostics_open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(translate("detail.sections.diagnostics"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.epoch == epoch {
                            view.diagnostics_open = !view.diagnostics_open;
                            cx.notify();
                        }
                    })),
            )
            .when(self.diagnostics_open, |section| {
                let mut rows = div()
                    .id("node-diagnostic-rows")
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap_2()
                    .when(pages > 1, |view| {
                        view.max_h(gpui::px(320.))
                            .overflow_y_scroll()
                            .track_scroll(&self.diagnostics_scroll)
                    });
                let projection = self.projection.as_ref().expect("selected node projection");
                for diagnostic in node
                    .diagnostics
                    .iter()
                    .skip(self.diagnostics_page * PAGE_DIAGNOSTICS)
                    .take(PAGE_DIAGNOSTICS)
                {
                    let (icon, color, key) = match diagnostic.severity {
                        EditorDiagnosticSeverity::Error => (
                            IconName::CircleX,
                            cx.theme().danger,
                            "native.workbench.diagnosticError",
                        ),
                        EditorDiagnosticSeverity::Warning => (
                            IconName::TriangleAlert,
                            cx.theme().warning,
                            "native.workbench.diagnosticWarning",
                        ),
                        EditorDiagnosticSeverity::Information => (
                            IconName::Info,
                            cx.theme().muted_foreground,
                            "native.workbench.diagnosticInformation",
                        ),
                    };
                    rows = rows.child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_shrink_0()
                                    .gap_1()
                                    .text_color(color)
                                    .child(Icon::new(icon).size_3())
                                    .child(translate(key)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(cx.theme().muted_foreground)
                                    .when_some(
                                        location_label(diagnostic, node, projection),
                                        |view, label| {
                                            view.child(
                                                div()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child(label),
                                            )
                                        },
                                    )
                                    .child(graph_diagnostic(diagnostic)),
                            ),
                    );
                }
                let mut body = div().flex().flex_col().min_w_0().p_2().gap_2().child(rows);
                if pages > 1 {
                    body = body.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Button::new("node-diagnostics-previous")
                                    .small()
                                    .ghost()
                                    .icon(IconName::ChevronLeft)
                                    .tooltip(translate("databaseEditor.previousPage"))
                                    .disabled(self.diagnostics_page == 0)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.epoch == epoch {
                                            view.diagnostics_page =
                                                view.diagnostics_page.saturating_sub(1);
                                            view.diagnostics_scroll
                                                .set_offset(gpui::Point::default());
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .child(format!("{} / {pages}", self.diagnostics_page + 1)),
                            )
                            .child(
                                Button::new("node-diagnostics-next")
                                    .small()
                                    .ghost()
                                    .icon(IconName::ChevronRight)
                                    .tooltip(translate("databaseEditor.nextPage"))
                                    .disabled(self.diagnostics_page + 1 >= pages)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.epoch == epoch {
                                            view.diagnostics_page =
                                                (view.diagnostics_page + 1).min(pages - 1);
                                            view.diagnostics_scroll
                                                .set_offset(gpui::Point::default());
                                            cx.notify();
                                        }
                                    })),
                            ),
                    );
                }
                section.content(body)
            })
            .into_any_element()
    }
}

fn location_label(
    diagnostic: &EditorDiagnosticModel,
    owner: &EditorNodeModel,
    projection: &EditorProjectionModel,
) -> Option<String> {
    let node = |id: NodeId| {
        if id == owner.node_id {
            Some(owner)
        } else {
            projection.nodes.iter().find(|node| node.node_id == id)
        }
    };
    let port = |address: &PortAddress| {
        let node = node(address.node_id)?;
        let port = node.ports.iter().find(|port| port.address == *address);
        Some(match port {
            Some(port) => format!(
                "{} · {}",
                node_title(node),
                port.display
                    .instance_label
                    .as_deref()
                    .unwrap_or(&port.display.label)
            ),
            None => node_title(node).to_owned(),
        })
    };
    let label = match &diagnostic.location {
        DiagnosticLocation::Graph => None,
        DiagnosticLocation::Node(id) => node(*id).map(|node| node_title(node).to_owned()),
        DiagnosticLocation::Resource(identity) => Some(identity.to_string()),
        DiagnosticLocation::Port(address) => port(address),
        DiagnosticLocation::Parameter { node_id, key } => node(*node_id).map(|node| {
            let parameter = node
                .parameter_groups
                .iter()
                .flat_map(|group| group.parameters.iter())
                .find(|parameter| parameter.key == *key);
            match parameter {
                Some(parameter) => format!("{} · {}", node_title(node), parameter.display.title),
                None => node_title(node).to_owned(),
            }
        }),
        DiagnosticLocation::Connection(id) => projection
            .connections
            .iter()
            .find(|connection| connection.connection_id == *id)
            .and_then(
                |connection| match (port(&connection.output), port(&connection.input)) {
                    (Some(output), Some(input)) => Some(format!("{output} → {input}")),
                    (output, input) => output.or(input),
                },
            ),
    };
    label.filter(|label| label != node_title(owner))
}

fn node_title(node: &EditorNodeModel) -> &str {
    node.display
        .user_label
        .as_deref()
        .unwrap_or(&node.display.title)
}
