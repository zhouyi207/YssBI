//! Resolve intents against the active canvas's exact displayed projection.
use super::LocateProblem;
use crate::{canvas::GraphCanvas, workbench::Workbench};
use gpui_kit::component::dock::{DockPlacement, panel_handle};
use gpui_kit::{App, Context, Entity, FocusHandle, Window};
use std::sync::Arc;
use yss_graph_analysis_contract::DiagnosticLocation;
use yss_project_identity::ProjectResourceKind;

impl Workbench {
    fn problem_graph(&self, event: &LocateProblem, cx: &App) -> Option<Entity<GraphCanvas>> {
        if self.is_closing(cx) {
            return None;
        }
        let graph = self
            .graphs
            .get(event.projection.graph_path.as_str())?
            .upgrade()?;
        if self
            .active_editor_panel(cx)
            .is_none_or(|panel| panel.view().entity_id() != graph.entity_id())
            || !Arc::ptr_eq(&event.projection, &graph.read(cx).graph.projection)
        {
            return None;
        }
        Some(graph)
    }

    pub(in crate::workbench) fn locate_problem(
        &mut self,
        event: &LocateProblem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.problem_graph(event, cx).is_none() {
            return;
        }
        let Some(location) = event.location() else {
            return;
        };
        if let DiagnosticLocation::Resource(identity) = location {
            let resource = self.project.as_ref().and_then(|project| {
                project
                    .resources
                    .entries
                    .iter()
                    .find(|entry| {
                        entry.resource.id == identity.as_ref()
                            || (entry.resource.kind == ProjectResourceKind::Database
                                && identity.strip_prefix("databases/")
                                    == Some(entry.resource.id.as_str()))
                    })
                    .map(|entry| entry.resource.clone())
            });
            if let Some(resource) = resource {
                self.open_project_resource(&resource, window, cx);
            }
            return;
        }
        let details_node = match location {
            DiagnosticLocation::Node(id) | DiagnosticLocation::Parameter { node_id: id, .. } => {
                Some(*id)
            }
            DiagnosticLocation::Port(address) => Some(address.node_id),
            _ => None,
        };
        if let Some(node_id) = details_node {
            let Some(node) = event
                .projection
                .nodes
                .iter()
                .find(|node| node.node_id == node_id)
            else {
                return;
            };
            if let DiagnosticLocation::Port(address) = location
                && !node.ports.iter().any(|port| port.address == *address)
            {
                return;
            }
            self.present_panel(
                panel_handle(self.details.clone()),
                DockPlacement::Right,
                window,
                cx,
            );
        }
        let owner = cx.entity().downgrade();
        let event = event.clone();
        let focus = window.focused(cx);
        // GPUI delivers frame callbacks before drawing. Stage one frame so the
        // Dock resize is painted before reading the canvas bounds.
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                let _ = owner.update(cx, |view, cx| {
                    view.apply_problem_location(&event, focus, window, cx)
                });
            });
        });
    }

    fn apply_problem_location(
        &mut self,
        event: &LocateProblem,
        focus: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.focused(cx) != focus {
            return;
        }
        let Some(graph) = self.problem_graph(event, cx) else {
            return;
        };
        let Some(location) = event.location() else {
            return;
        };
        let applied = graph.update(cx, |graph, cx| match location {
            DiagnosticLocation::Graph => {
                graph.reveal_graph(window, cx);
                true
            }
            DiagnosticLocation::Node(id) | DiagnosticLocation::Parameter { node_id: id, .. } => {
                graph.reveal_node(*id, window, cx)
            }
            DiagnosticLocation::Port(address) => graph.reveal_port(address, window, cx),
            DiagnosticLocation::Connection(id) => graph.reveal_connection(*id, window, cx),
            DiagnosticLocation::Resource(_) => false,
        });
        if !applied {
            return;
        }
        if let DiagnosticLocation::Parameter { node_id, key } = location {
            let node_id = *node_id;
            let key = key.clone();
            let event = event.clone();
            // The existing Canvas subscription installs the selection before this callback.
            cx.defer_in(window, move |view, window, cx| {
                if view.problem_graph(&event, cx).is_none() {
                    return;
                }
                view.details.update(cx, |details, cx| {
                    details.reveal_parameter(node_id, &key, window, cx)
                });
            });
        }
    }
}
