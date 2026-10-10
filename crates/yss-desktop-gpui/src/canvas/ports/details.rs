//! Index immutable projection facts once; format only the port whose tooltip is opened.
use gpui::{AnyView, App, AppContext, WeakEntity, div, prelude::*, px};
use gpui_component::tooltip::Tooltip;
use std::{collections::BTreeMap, sync::Arc};
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_graph_document::PortAddress;
use yss_graph_editor::projection::{
    EditorDiagnosticModel, EditorPortModel, EditorPortTypeState, EditorProjectionModel,
};

use crate::{canvas::presentation::PortAppearance, text};

struct PortDetails {
    node: usize,
    port: usize,
    diagnostic: Option<usize>,
    sources: Vec<(usize, usize)>,
}

pub(in crate::canvas) struct Details {
    pub projection: Arc<EditorProjectionModel>,
    ports: BTreeMap<PortAddress, PortDetails>,
}

impl Details {
    pub fn new(projection: Arc<EditorProjectionModel>) -> Self {
        let mut ports = projection
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(node, model)| {
                model.ports.iter().enumerate().map(move |(port, model)| {
                    (
                        model.address.clone(),
                        PortDetails {
                            node,
                            port,
                            diagnostic: None,
                            sources: Vec::new(),
                        },
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        for (index, diagnostic) in projection.diagnostics.iter().enumerate() {
            if let DiagnosticLocation::Port(address) = &diagnostic.location
                && let Some(port) = ports.get_mut(address)
                && port.diagnostic.is_none_or(|previous| {
                    diagnostic.blocking && !projection.diagnostics[previous].blocking
                })
            {
                port.diagnostic = Some(index);
            }
        }
        for connection in &projection.connections {
            let Some(source) = ports
                .get(&connection.output)
                .map(|port| (port.node, port.port))
            else {
                continue;
            };
            if let Some(port) = ports.get_mut(&connection.input) {
                port.sources.push(source);
            }
        }
        Self { projection, ports }
    }

    pub fn port(&self, address: &PortAddress) -> Option<&EditorPortModel> {
        let details = self.ports.get(address)?;
        Some(&self.projection.nodes[details.node].ports[details.port])
    }

    pub fn diagnostic(&self, address: &PortAddress) -> Option<&EditorDiagnosticModel> {
        let index = self.ports.get(address)?.diagnostic?;
        self.projection.diagnostics.get(index)
    }

    pub fn tooltip(&self, address: &PortAddress, appearance: PortAppearance) -> String {
        let Some(details) = self.ports.get(address) else {
            return String::new();
        };
        let port = &self.projection.nodes[details.node].ports[details.port];
        let label = port
            .display
            .instance_label
            .as_deref()
            .unwrap_or(&port.display.label);
        let data_type = match &port.type_state {
            EditorPortTypeState::Exact { display, .. }
            | EditorPortTypeState::Constrained { display, .. } => display,
            EditorPortTypeState::Unknown { .. } => text::t("native.canvas.portTypeUnknown"),
            EditorPortTypeState::Conflict { .. } => text::t("native.canvas.portTypeConflict"),
        };
        let mut lines = vec![format!("{label} ({data_type})")];
        if let Some(diagnostic) = self.diagnostic(address) {
            lines.push(text::graph_diagnostic(diagnostic));
        }
        for &(node, port) in &details.sources {
            let node = &self.projection.nodes[node];
            let port = &node.ports[port];
            let title = node
                .display
                .user_label
                .as_deref()
                .unwrap_or(&node.display.title);
            let label = port
                .display
                .instance_label
                .as_deref()
                .unwrap_or(&port.display.label);
            lines.push(format!("{title} · {label}"));
        }
        lines.push(appearance.state.label().to_owned());
        if appearance.state == super::State::Error && appearance.cache != super::State::Unexecuted {
            lines.push(appearance.cache.label().to_owned());
        }
        lines.join("\n")
    }
}

// Only an open tooltip observes the canvas. Its content owns the subscription,
// so closing it releases both the observation and its weak graph reference.
pub(super) fn tooltip(
    owner: WeakEntity<crate::canvas::GraphCanvas>,
    address: PortAddress,
    cx: &mut App,
) -> AnyView {
    cx.new(|cx| {
        let observation = owner
            .upgrade()
            .map(|owner| cx.observe(&owner, |_, _, cx| cx.notify()));
        Tooltip::element(move |_, cx| {
            let content = observation
                .as_ref()
                .and_then(|_| owner.upgrade())
                .and_then(|owner| {
                    let view = owner.read(cx);
                    let appearance = view
                        .presentation
                        .ports
                        .get(&address)
                        .copied()
                        .unwrap_or_default();
                    view.port_details
                        .as_ref()
                        .map(|details| details.tooltip(&address, appearance))
                })
                .unwrap_or_default();
            div().max_w(px(360.)).child(content)
        })
        .max_w(px(360.))
    })
    .into()
}
