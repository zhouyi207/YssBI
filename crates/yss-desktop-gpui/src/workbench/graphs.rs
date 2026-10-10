mod loading;
mod navigation;
mod opening;
mod resources;
pub(super) use opening::Opening;

use gpui::AppContext;
use gpui::{Context, Window};
use gpui_component::dock::{DockPlacement, InsertTarget};
use yss_graph_document::GraphResourcePath;

use super::Workbench;
use crate::{
    canvas::{CanvasEvent, GraphCanvas},
    project::OpenedGraph,
};

impl Workbench {
    pub(super) fn open_graph(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_graph_requested(path, None, None, window, cx);
    }

    fn open_dropped_graph(
        &mut self,
        target_canvas: &gpui::Entity<GraphCanvas>,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .project
            .as_ref()
            .is_none_or(|project| project.identity != target_canvas.read(cx).graph.project)
        {
            return;
        }
        let panel_id = target_canvas.entity_id().into();
        let Some(placement) = self.displayed_panel_placement(panel_id, cx) else {
            return;
        };
        let target = self
            .dock
            .read(cx)
            .layout(placement)
            .and_then(|tree| tree.find_panel_node(panel_id));
        let existing = self
            .graphs
            .get(path)
            .and_then(gpui::WeakEntity::upgrade)
            .is_some()
            || self
                .graph_openings
                .get(path)
                .and_then(gpui::WeakEntity::upgrade)
                .is_some();
        self.open_graph(path.to_owned(), window, cx);
        // A new loading tab already owns the asynchronous read. Move that tab
        // now; its normal replacement installs the canvas in the same group.
        if !existing
            && let Some(target) = target
            && let Some(opening) = self
                .graph_openings
                .get(path)
                .and_then(gpui::WeakEntity::upgrade)
        {
            self.dock.update(cx, |dock, cx| {
                dock.move_panel(
                    opening.entity_id().into(),
                    InsertTarget::Tabs {
                        node: target,
                        ix: None,
                        activate: true,
                    },
                    window,
                    cx,
                );
            });
        }
    }

    pub(super) fn open_graph_intent(
        &mut self,
        path: String,
        node: Option<String>,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_graph_requested(path, node, Some(id), window, cx);
    }

    pub(super) fn focus_graph(
        &mut self,
        path: String,
        node: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_graph_requested(path, node, None, window, cx);
    }

    fn open_graph_requested(
        &mut self,
        path: String,
        node: Option<String>,
        intent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing {
            return;
        }
        self.start_graph_read(path, node, intent, window, cx);
    }

    fn start_graph_read(
        &mut self,
        path: String,
        node: Option<String>,
        intent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.project.is_none() {
            return;
        }
        if let Some(graph) = self.graphs.get(&path).and_then(gpui::WeakEntity::upgrade) {
            self.present_panel(
                gpui_component::dock::panel_handle(graph.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
            if let Some(id) = intent {
                self.reveal_graph_intent(graph, node, id, window, cx);
            } else if self
                .displayed_panel_placement(graph.entity_id().into(), cx)
                .is_some()
            {
                graph.update(cx, |graph, cx| {
                    graph.focus_node(node.as_deref(), window, cx)
                });
            }
            return;
        }
        let graph_path = match GraphResourcePath::new(path.clone()) {
            Ok(path) => path,
            Err(_) => {
                self.error = Some(crate::text::t("native.workbench.invalidGraphPath").into());
                cx.notify();
                if let Some(id) = intent {
                    self.finish_intent(&id, false, window, cx);
                }
                return;
            }
        };
        let opening = self
            .graph_openings
            .get(&path)
            .and_then(gpui::WeakEntity::upgrade)
            .filter(|opening| !opening.read(cx).removed)
            .unwrap_or_else(|| self.create_graph_opening(graph_path, window, cx));
        if let Some(intent) = intent {
            if opening.read(cx).intent.is_some() {
                self.finish_intent(&intent, false, window, cx);
            } else {
                opening.update(cx, |opening, _| {
                    // A pending first read may contain the projection from before the intent.
                    opening.task = None;
                    opening.intent = Some(intent);
                    opening.node = node;
                });
            }
        } else if node.is_some() {
            opening.update(cx, |opening, _| opening.node = node);
        }
        self.present_panel(
            gpui_component::dock::panel_handle(opening.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        window.focus(&gpui::Focusable::focus_handle(&opening, cx), cx);
        self.read_graph_opening(opening, window, cx);
    }

    fn build_graph(
        &mut self,
        graph: OpenedGraph,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<GraphCanvas>> {
        if let Some(canvas) = self
            .graphs
            .get(graph.projection.graph_path.as_str())
            .and_then(gpui::WeakEntity::upgrade)
        {
            return Some(canvas);
        }
        let catalog = self
            .project
            .as_ref()
            .and_then(|project| {
                project
                    .panels
                    .iter()
                    .find(|panel| panel.panel_id == "nodes")
            })
            .cloned()?;
        let path = graph.projection.graph_path.as_str().to_owned();
        let services = self.services.clone();
        let catalog_language = self.project.as_ref()?.language.clone();
        let canvas = cx.new(|cx| {
            let mut canvas =
                GraphCanvas::new(services, graph, catalog, catalog_language, window, cx);
            canvas.bind_viewport(self.layout_root.clone());
            canvas
        });
        self.observe_graph_status(&canvas, cx);
        let mut diagnostic_count = canvas.read(cx).graph.projection.diagnostics.len();
        self.subscriptions.push(cx.subscribe_in(
            &canvas,
            window,
            move |view, canvas, event, window, cx| {
                match event {
                    CanvasEvent::ResourceLocated { path, version } => {
                        view.recover_graph_resource(
                            canvas.clone(),
                            path.clone(),
                            *version,
                            window,
                            cx,
                        );
                    }
                    CanvasEvent::Selection { nodes, projection }
                    | CanvasEvent::Projection { nodes, projection } => {
                        // A group's active tab can still be hidden behind another zoomed group.
                        if matches!(event, CanvasEvent::Selection { .. })
                            && view
                                .displayed_panel_placement(canvas.entity_id().into(), cx)
                                .is_none()
                        {
                            return;
                        }
                        if diagnostic_count != projection.diagnostics.len() {
                            diagnostic_count = projection.diagnostics.len();
                            view.refresh_resource_rows(cx);
                        }
                        if matches!(event, CanvasEvent::Projection { .. })
                            && view
                                .details
                                .read(cx)
                                .graph()
                                .is_none_or(|current| current.entity_id() != canvas.entity_id())
                        {
                            return;
                        }
                        let version = canvas.read(cx).graph.editing.version;
                        let path = canvas.read(cx).path().to_owned();
                        view.mark_project_resource(Some(&path), cx);
                        view.details.update(cx, |details, cx| {
                            if matches!(event, CanvasEvent::Selection { .. }) {
                                details.clear_log(cx);
                            }
                            details.set_selection(
                                canvas.downgrade(),
                                nodes.clone(),
                                projection.clone(),
                                version,
                                window,
                                cx,
                            );
                            if matches!(event, CanvasEvent::Selection { .. }) {
                                details.show_node_properties(cx);
                            }
                        });
                        view.problems.update(cx, |problems, cx| {
                            problems.set_projection(projection.clone(), cx)
                        });
                        view.output.update(cx, |output, cx| {
                            output.set_graph(Some(canvas.downgrade()), cx)
                        });
                        view.results.update(cx, |results, cx| {
                            results.set_graph(Some(canvas.read(cx)));
                            cx.notify();
                        });
                    }
                    CanvasEvent::RevealNodeDetails => {
                        if view
                            .details
                            .read(cx)
                            .graph()
                            .is_some_and(|graph| graph.entity_id() == canvas.entity_id())
                        {
                            view.present_panel(
                                gpui_component::dock::panel_handle(view.details.clone()),
                                gpui_component::dock::DockPlacement::Right,
                                window,
                                cx,
                            );
                            window.focus(&gpui::Focusable::focus_handle(canvas.read(cx), cx), cx);
                        }
                    }
                    CanvasEvent::Execution => {
                        if view
                            .details
                            .read(cx)
                            .graph()
                            .is_some_and(|graph| graph.entity_id() == canvas.entity_id())
                        {
                            view.results.update(cx, |results, cx| {
                                results.set_graph(Some(canvas.read(cx)));
                                cx.notify();
                            });
                        }
                        cx.notify();
                    }
                    CanvasEvent::ShowOutput => {
                        if view
                            .details
                            .read(cx)
                            .graph()
                            .is_some_and(|graph| graph.entity_id() == canvas.entity_id())
                        {
                            view.present_panel(
                                gpui_component::dock::panel_handle(view.output.clone()),
                                DockPlacement::Bottom,
                                window,
                                cx,
                            );
                        }
                    }
                    CanvasEvent::ShowResults => {
                        view.present_panel(
                            gpui_component::dock::panel_handle(view.results.clone()),
                            DockPlacement::Bottom,
                            window,
                            cx,
                        );
                    }
                    CanvasEvent::InspectResult(lease) => {
                        if view.project.as_ref().is_some_and(|project| {
                            project.identity == canvas.read(cx).graph.project
                        }) && view
                            .active_editor_panel(cx)
                            .is_some_and(|panel| panel.view().entity_id() == canvas.entity_id())
                        {
                            view.open_result(
                                lease.reference(),
                                None,
                                Some(lease.clone()),
                                window,
                                cx,
                            );
                        }
                    }
                    CanvasEvent::Edited => cx.notify(),
                    CanvasEvent::OpenGraph(path) => {
                        view.open_dropped_graph(canvas, path, window, cx)
                    }
                }
            },
        ));
        self.graphs.insert(path, canvas.downgrade());
        self.refresh_resource_rows(cx);
        Some(canvas)
    }
}
