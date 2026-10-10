//! Projection reads retain the last view and drafts; only successful reads restore editing.
use super::{CanvasEvent, GraphCanvas};
use gpui::Context;

impl GraphCanvas {
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.refresh_task.is_some() {
            self.refresh_pending = true;
            return;
        }
        self.refresh_pending = false;
        let version = self.graph.editing.version;
        let path = self.graph.projection.graph_path.clone();
        let language = crate::text::locale();
        let request = yss_application::graph::open::OpenGraphRequest::new(
            self.graph.project.clone(),
            self.graph.projection.graph_path.clone(),
            0,
            language,
        );
        let task = self.services.run(move |services| {
            Ok(crate::project::OpenedGraph::from_open(
                services.application.open_graph(request)?,
            ))
        });
        self.refresh_task = Some(cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.refresh_task = None;
                if view.graph.projection.graph_path != path {
                    return;
                }
                if view.busy
                    || view.graph.editing.version != version
                    || language != crate::text::locale()
                {
                    view.refresh_pending = true;
                } else {
                    match result {
                        Ok(graph) => {
                            view.install_projection(graph, cx);
                        }
                        Err(_error) => {
                            tracing::warn!(
                                code = "native_graph_refresh_failed",
                                "Native graph refresh failed"
                            );
                            view.refresh_failed = true;
                            view.palette = None;
                            view.cancel_gesture();
                            view.emit_selection(cx);
                        }
                    }
                }
                if view.refresh_pending && !view.busy {
                    view.refresh(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(crate) fn install_projection(
        &mut self,
        graph: crate::project::OpenedGraph,
        cx: &mut Context<Self>,
    ) {
        self.refresh_task = None;
        self.refresh_failed = false;
        self.graph.replace(graph);
        *self.connection_layer.borrow_mut() =
            super::connections::ConnectionLayer::new(&self.graph.projection);
        self.refresh_presentation();
        self.cancel_gesture();
        self.retain_located();
        self.selected.retain(|id| {
            self.graph
                .projection
                .nodes
                .iter()
                .any(|node| node.node_id == *id)
        });
        self.resync_execution(cx);
        cx.emit(CanvasEvent::Projection {
            nodes: self.selected.iter().copied().collect(),
            projection: self.graph.projection.clone(),
        });
        cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
        cx.notify();
    }
}
