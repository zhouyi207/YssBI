//! UI intents await the canvas's current read before locating a node or reporting success.
use super::super::Workbench;
use crate::canvas::GraphCanvas;
use gpui::{Context, Entity, Window};

impl Workbench {
    pub(super) fn reveal_graph_intent(
        &mut self,
        canvas: Entity<GraphCanvas>,
        node: Option<String>,
        intent: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = canvas.read(cx).graph.project.clone();
        let path = canvas.read(cx).path().to_owned();
        let lifecycle = self.lifecycle;
        let delivery = self.ui_delivery;
        let refresh = canvas.update(cx, |canvas, cx| {
            canvas.focus_node(None, window, cx);
            canvas.refresh_for_navigation(cx)
        });
        let canvas = canvas.downgrade();
        cx.spawn_in(window, async move |view, cx| {
            let refreshed = refresh.await;
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view.ui_delivery != delivery
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|current| current.identity != project)
                {
                    return;
                }
                let applied = !view.is_closing(cx)
                    && canvas.upgrade().is_some_and(|canvas| {
                        view.graphs
                            .get(&path)
                            .is_some_and(|held| held.entity_id() == canvas.entity_id())
                            && canvas.read(cx).path() == path
                            && view
                                .active_editor_panel(cx)
                                .is_some_and(|panel| panel.view().entity_id() == canvas.entity_id())
                            && refreshed.is_some_and(|state| {
                                canvas.read(cx).navigation_state() == Some(state)
                            })
                            && canvas.update(cx, |canvas, cx| {
                                canvas.focus_node(node.as_deref(), window, cx)
                            })
                    });
                view.finish_intent(&intent, applied, window, cx);
            });
        })
        .detach();
    }
}
