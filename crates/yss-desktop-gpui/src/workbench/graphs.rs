mod resources;

use gpui::AppContext;
use gpui::{Context, Window};
use gpui_component::dock::DockPlacement;
use yss_application::graph::open::OpenGraphRequest;
use yss_graph_document::GraphResourcePath;

use super::Workbench;
use crate::{
    canvas::{CanvasEvent, GraphCanvas},
    project::OpenedGraph,
};

pub(super) struct Opening {
    node: Option<String>,
    intent: Option<String>,
    revision: Option<yss_project_identity::ResourceRevision>,
    _task: gpui::Task<()>,
}

impl Workbench {
    pub(super) fn open_graph(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_graph_requested(path, None, None, window, cx);
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
        let Some(project) = &self.project else {
            return;
        };
        if let Some(graph) = self.graphs.get(&path).and_then(gpui::WeakEntity::upgrade) {
            let applied = graph.update(cx, |graph, cx| {
                graph.focus_node(node.as_deref(), window, cx)
            });
            self.present_panel(
                gpui_component::dock::panel_handle(graph),
                DockPlacement::Center,
                window,
                cx,
            );
            if let Some(id) = intent {
                self.finish_intent(&id, applied, window, cx);
            }
            return;
        }
        if self.graph_openings.contains_key(&path) {
            if let Some(id) = intent {
                self.finish_intent(&id, false, window, cx);
            }
            return;
        }
        let identity = project.identity.clone();
        let lifecycle = self.lifecycle;
        let graph_path = match GraphResourcePath::new(path.clone()) {
            Ok(path) => path,
            Err(_) => {
                self.error = Some("图路径无效".into());
                cx.notify();
                if let Some(id) = intent {
                    self.finish_intent(&id, false, window, cx);
                }
                return;
            }
        };
        let language = crate::text::locale();
        let request = OpenGraphRequest::new(identity.clone(), graph_path, 0, language);
        let task = self.services.run(move |services| {
            Ok(OpenedGraph::from_open(
                services.application.open_graph(request)?,
            ))
        });
        let source = path.clone();
        let revision = self.indexed_graph_revision(&path);
        let delivery = cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != identity)
                {
                    return;
                }
                let Some(opening) = view.graph_openings.remove(&path) else {
                    return;
                };
                let Opening { node, intent, .. } = opening;
                if language != crate::text::locale() {
                    view.start_graph_read(path, node, intent, window, cx);
                    return;
                }
                let applied = match result {
                    Ok(graph) => view.install_graph(graph, window, cx).is_some_and(|graph| {
                        graph.update(cx, |graph, cx| {
                            graph.focus_node(node.as_deref(), window, cx)
                        })
                    }),
                    Err(_error) => {
                        tracing::error!(
                            code = "native_graph_open_failed",
                            "Native graph open failed"
                        );
                        view.error = Some("无法打开图，请检查项目资源。".into());
                        false
                    }
                };
                if let Some(id) = intent {
                    view.finish_intent(&id, applied, window, cx);
                }
                cx.notify();
            });
        });
        self.graph_openings.insert(
            source,
            Opening {
                node,
                intent,
                revision,
                _task: delivery,
            },
        );
    }

    pub(super) fn install_graph(
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
            self.present_panel(
                gpui_component::dock::panel_handle(canvas.clone()),
                DockPlacement::Center,
                window,
                cx,
            );
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
        let catalog_language = crate::text::locale().to_owned();
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
                    CanvasEvent::Selection { nodes, projection }
                    | CanvasEvent::Projection { nodes, projection } => {
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
                            .active_editor_panel(cx)
                            .is_some_and(|panel| panel.view().entity_id() == canvas.entity_id())
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
                    CanvasEvent::OpenGraph(path) => view.open_graph(path.clone(), window, cx),
                }
            },
        ));
        self.graphs.insert(path, canvas.downgrade());
        self.refresh_resource_rows(cx);
        self.present_panel(
            gpui_component::dock::panel_handle(canvas.clone()),
            DockPlacement::Center,
            window,
            cx,
        );
        Some(canvas)
    }
}
