//! Projection reads retain the last view and drafts; only successful reads restore editing.
use super::{CanvasEvent, GraphCanvas};
use gpui::Context;
use yss_graph_document::GraphResourcePath;
use yss_project::GraphEditVersion;
use yss_project_identity::ResourceRevision;

pub(super) struct ResourceMove {
    path: GraphResourcePath,
    revision: ResourceRevision,
    draft_base: Option<GraphEditVersion>,
}

impl GraphCanvas {
    pub(super) fn resource_path(&self) -> &GraphResourcePath {
        self.resource_move
            .as_ref()
            .map_or(&self.graph.projection.graph_path, |moved| &moved.path)
    }

    pub(crate) fn resource_revision(&self) -> ResourceRevision {
        self.resource_move
            .as_ref()
            .map_or(self.graph.editing.version.revision, |moved| moved.revision)
    }

    pub(crate) fn move_resource(
        &mut self,
        path: GraphResourcePath,
        from: ResourceRevision,
        to: ResourceRevision,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.resource_revision() > from {
            return false;
        }
        let draft_base = if let Some(moved) = &self.resource_move {
            (moved.revision == from)
                .then_some(moved.draft_base)
                .flatten()
        } else {
            (self.graph.editing.version.revision == from).then_some(self.graph.editing.version)
        };
        self.bind_resource(path, to, draft_base, cx);
        true
    }

    pub(crate) fn recover_resource(
        &mut self,
        path: GraphResourcePath,
        version: GraphEditVersion,
        cx: &mut Context<Self>,
    ) -> bool {
        if version.session_id != self.graph.editing.version.session_id
            || version.revision < self.resource_revision()
            || &path == self.resource_path()
        {
            return false;
        }
        // Session identity proves location, not the missing edits between the two revisions.
        self.bind_resource(path, version.revision, None, cx);
        true
    }

    fn bind_resource(
        &mut self,
        path: GraphResourcePath,
        revision: ResourceRevision,
        draft_base: Option<GraphEditVersion>,
        cx: &mut Context<Self>,
    ) {
        self.refresh_task = None;
        self.cancel_gesture();
        self.palette = None;
        self.resource_move = Some(ResourceMove {
            path,
            revision,
            draft_base,
        });
        self.refresh_failed = false;
        self.refresh(cx);
        cx.emit(CanvasEvent::Projection {
            nodes: self.selected.iter().copied().collect(),
            projection: self.graph.projection.clone(),
        });
        cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
        cx.notify();
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.refresh_task.is_some() {
            self.refresh_pending = true;
            return;
        }
        self.refresh_pending = false;
        let version = self.graph.editing.version;
        let path = self.resource_path().clone();
        let language = crate::text::locale();
        let request = yss_application::graph::open::OpenGraphRequest::new(
            self.graph.project.clone(),
            path.clone(),
            0,
            language,
        );
        let task = self.services.run(move |services| {
            Ok(crate::project::OpenedGraph::from_open(
                services
                    .application
                    .refresh_graph(request, version.session_id)?,
            ))
        });
        self.refresh_task = Some(cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.refresh_task = None;
                if view.resource_path() != &path {
                    return;
                }
                if view.busy
                    || view.graph.editing.version != version
                    || language != crate::text::locale()
                {
                    view.refresh_pending = true;
                } else {
                    match result {
                        Ok(graph)
                            if graph.project == view.graph.project
                                && graph.editing.version.session_id == version.session_id
                                && &graph.projection.graph_path != view.resource_path() =>
                        {
                            view.refresh_failed = true;
                            view.refresh_pending = false;
                            view.palette = None;
                            view.cancel_gesture();
                            cx.emit(CanvasEvent::ResourceLocated {
                                path: graph.projection.graph_path.clone(),
                                version: graph.editing.version,
                            });
                        }
                        Ok(graph) if view.accepts_projection(&graph) => {
                            view.install_projection(graph, cx);
                        }
                        _ => {
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

    fn accepts_projection(&self, graph: &crate::project::OpenedGraph) -> bool {
        graph.project == self.graph.project
            && &graph.projection.graph_path == self.resource_path()
            && self.resource_move.as_ref().is_none_or(|moved| {
                graph.editing.version.revision >= moved.revision
                    && graph.editing.version.session_id == self.graph.editing.version.session_id
            })
    }

    fn install_projection(&mut self, graph: crate::project::OpenedGraph, cx: &mut Context<Self>) {
        self.refresh_task = None;
        self.refresh_failed = false;
        let draft_base = self.resource_move.take().and_then(|moved| {
            (graph.editing.version.revision == moved.revision)
                .then_some(moved.draft_base)
                .flatten()
        });
        let previous_ports = self.port_details.clone();
        self.graph.replace(graph);
        *self.connection_layer.borrow_mut() =
            super::connections::ConnectionLayer::new(&self.graph.projection);
        self.refresh_presentation();
        if let (Some(base), Some(previous_ports)) = (draft_base, previous_ports) {
            self.rebind_moved_port_inputs(base, &previous_ports);
        }
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
