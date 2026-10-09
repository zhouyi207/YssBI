//! Platform clipboard transport uses the editor's bounded subgraph contract directly.
use super::{GraphCanvas, GraphCommand};
use gpui::{ClipboardItem, Context};
use std::sync::Arc;
use yss_graph_editor::EditorGraphMutation;

impl GraphCanvas {
    pub(super) fn copy_selection(&mut self, cut: bool, cx: &mut Context<Self>) {
        if !self.can_copy_selection() {
            return;
        }
        self.cancel_gesture();
        let projection = self.graph.projection.clone();
        let version = self.graph.editing.version;
        let project = self.graph.project.clone();
        let path = projection.graph_path.clone();
        let selected = self.selected.clone();
        let nodes = selected.iter().copied().collect();
        let task = self.services.run(move |services| {
            let snapshot = services
                .application
                .export_graph_subgraph(&project, &path, version, nodes)?;
            Ok(serde_json::to_string(&snapshot)?)
        });
        self.clipboard_task = Some(cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                view.clipboard_task = None;
                if view.busy
                    || view.graph.editing.version != version
                    || !Arc::ptr_eq(&view.graph.projection, &projection)
                    || view.selected != selected
                {
                    return;
                }
                match result {
                    Ok(source) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(source));
                        if cut {
                            view.submit(
                                GraphCommand::Edit(EditorGraphMutation::DeleteNodes {
                                    node_ids: selected.into_iter().collect(),
                                }),
                                Some(version),
                                cx,
                            );
                        }
                    }
                    Err(_) => {
                        tracing::warn!(
                            code = "native_graph_copy_failed",
                            "Graph clipboard export failed"
                        );
                        view.error = Some(crate::text::translate("panel.assistantCopyFailed"));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn paste_selection(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancel_gesture();
        let projection = self.graph.projection.clone();
        let version = self.graph.editing.version;
        let anchor = self.world(self.bounds.get().center());
        let task = cx.read_from_clipboard_async();
        self.clipboard_task = Some(cx.spawn(async move |view, cx| {
            let result = task.await;
            let _ = view.update(cx, |view, cx| {
                view.clipboard_task = None;
                if view.busy
                    || view.graph.editing.version != version
                    || !Arc::ptr_eq(&view.graph.projection, &projection)
                {
                    return;
                }
                match result.ok().flatten().and_then(|item| item.text()) {
                    Some(source) => {
                        view.submit(GraphCommand::Paste { source, anchor }, Some(version), cx)
                    }
                    None => {
                        view.error = Some(crate::text::translate("native.canvas.commandFailed"))
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
