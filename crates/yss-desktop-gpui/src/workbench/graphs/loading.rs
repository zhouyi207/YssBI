//! First reads belong to their tab; closing it cancels delivery without reopening it later.
use super::super::Workbench;
use super::{Opening, opening::OpeningEvent};
use crate::project::OpenedGraph;
use gpui::{AppContext, Context, Entity, Focusable, Window};
use gpui_component::dock::{PanelId, panel_handle};
use yss_application::graph::open::OpenGraphRequest;
use yss_graph_document::GraphResourcePath;

impl Workbench {
    pub(in crate::workbench) fn create_graph_opening(
        &mut self,
        path: GraphResourcePath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Opening> {
        let revision = self.indexed_graph_revision(path.as_str());
        let opening = cx.new(|cx| Opening::new(path, revision, cx));
        let lifecycle = self.lifecycle;
        self.subscriptions.push(cx.subscribe_in(
            &opening,
            window,
            move |view, opening, event, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                let path = opening.read(cx).path.as_str().to_owned();
                if view
                    .graph_openings
                    .get(&path)
                    .is_none_or(|held| held.entity_id() != opening.entity_id())
                {
                    return;
                }
                match event {
                    OpeningEvent::Removed(intent) => {
                        view.graph_openings.remove(&path);
                        if let Some(intent) = intent {
                            view.finish_intent(intent, false, window, cx);
                        }
                    }
                    OpeningEvent::Activated => {
                        if view
                            .displayed_panel_placement(opening.entity_id().into(), cx)
                            .is_some()
                        {
                            view.clear_graph_context(cx);
                            view.mark_project_resource(Some(&path), cx);
                        }
                        if !opening.read(cx).failed {
                            view.read_graph_opening(opening.clone(), window, cx);
                        }
                    }
                    OpeningEvent::Retry => {
                        if !view.is_closing(cx) {
                            view.read_graph_opening(opening.clone(), window, cx);
                        }
                    }
                }
            },
        ));
        self.graph_openings.insert(
            opening.read(cx).path.as_str().to_owned(),
            opening.downgrade(),
        );
        opening
    }

    pub(super) fn read_graph_opening(
        &mut self,
        opening: Entity<Opening>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if opening.read(cx).removed || opening.read(cx).task.is_some() {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let identity = project.identity.clone();
        let lifecycle = self.lifecycle;
        let path = opening.read(cx).path.clone();
        let language = crate::text::locale();
        let request = OpenGraphRequest::new(identity.clone(), path.clone(), 0, language);
        let task = self.services.run(move |services| {
            Ok(OpenedGraph::from_open(
                services.application.open_graph(request)?,
            ))
        });
        let target = opening.downgrade();
        let delivery = cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                let Some(opening) = target.upgrade() else {
                    return;
                };
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != identity)
                    || opening.read(cx).removed
                    || opening.read(cx).path != path
                    || view
                        .graph_openings
                        .get(path.as_str())
                        .is_none_or(|held| held.entity_id() != opening.entity_id())
                    || view
                        .panel_placement(PanelId::from(opening.entity_id()), cx)
                        .is_none()
                {
                    return;
                }
                opening.update(cx, |opening, _| opening.task = None);
                if language != crate::text::locale() {
                    view.read_graph_opening(opening, window, cx);
                    return;
                }
                let displayed = view
                    .displayed_panel_placement(PanelId::from(opening.entity_id()), cx)
                    .is_some();
                let focused = opening.focus_handle(cx).contains_focused(window, cx);
                let canvas = result
                    .ok()
                    .and_then(|graph| view.build_graph(graph, window, cx));
                let intent = opening.update(cx, |opening, _| opening.intent.take());
                let applied = if let Some(canvas) = canvas {
                    let node = opening.read(cx).node.clone();
                    view.graph_openings.remove(path.as_str());
                    let installed = view.replace_panel(
                        opening.clone(),
                        panel_handle(canvas.clone()),
                        window,
                        cx,
                    );
                    if installed && displayed && (focused || intent.is_some()) {
                        canvas.update(cx, |canvas, cx| {
                            canvas.focus_node(node.as_deref(), window, cx)
                        })
                    } else {
                        installed && displayed && node.is_none()
                    }
                } else {
                    tracing::warn!(
                        code = "native_graph_open_failed",
                        "Native graph open failed"
                    );
                    opening.update(cx, |opening, cx| {
                        opening.failed = true;
                        cx.notify();
                    });
                    false
                };
                if let Some(intent) = intent {
                    view.finish_intent(&intent, applied, window, cx);
                }
                cx.notify();
            });
        });
        opening.update(cx, |opening, cx| {
            opening.failed = false;
            opening.task = Some(delivery);
            cx.notify();
        });
    }
}
