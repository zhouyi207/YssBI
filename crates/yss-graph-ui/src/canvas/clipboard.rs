//! Platform clipboard transport uses the editor's bounded subgraph contract directly.
use super::{GraphCanvas, GraphCommand};
use gpui_kit::{ClipboardItem, Context};
use std::sync::Arc;
use yss_graph_editor::EditorGraphMutation;
use yss_ui::lifecycle::LogReleased;

/// Editing intent shared by graph menus, toolbar controls and keyboard actions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GraphEditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
}

impl GraphCanvas {
    pub fn is_edit_command_enabled(&self, command: GraphEditCommand) -> bool {
        if !self.can_edit() || self.read_task.is_some() {
            return false;
        }
        match command {
            GraphEditCommand::Undo => self.graph.editing.can_undo,
            GraphEditCommand::Redo => self.graph.editing.can_redo,
            GraphEditCommand::Cut | GraphEditCommand::Copy => self.can_copy_selection(),
            GraphEditCommand::Paste => true,
            GraphEditCommand::Delete => {
                !self.selected_connections.is_empty()
                    || self.graph.projection.nodes.iter().any(|node| {
                        self.selected.contains(&node.node_id) && !node.capabilities.managed
                    })
            }
        }
    }

    pub fn edit(&mut self, command: GraphEditCommand, cx: &mut Context<Self>) {
        if !self.is_edit_command_enabled(command) {
            return;
        }
        match command {
            GraphEditCommand::Undo => self.submit(GraphCommand::Undo, None, cx),
            GraphEditCommand::Redo => self.submit(GraphCommand::Redo, None, cx),
            GraphEditCommand::Cut => self.copy_selection(true, cx),
            GraphEditCommand::Copy => self.copy_selection(false, cx),
            GraphEditCommand::Paste => self.paste_selection(cx),
            GraphEditCommand::Delete => self.delete_selection(cx),
        }
    }

    fn copy_selection(&mut self, cut: bool, cx: &mut Context<Self>) {
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
            let snapshot = services.export_graph_subgraph(&project, &path, version, nodes)?;
            Ok(serde_json::to_string(&snapshot)?)
        });
        self.read_task = Some(cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update(cx, |view, cx| {
                view.read_task = None;
                if !view.can_edit()
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
                        view.notifications
                            .error(yss_ui::text::translate("panel.assistantCopyFailed"), cx);
                    }
                }
                cx.notify();
            })
            .log_released();
        }));
        cx.notify();
    }

    fn paste_selection(&mut self, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        self.cancel_gesture();
        let projection = self.graph.projection.clone();
        let version = self.graph.editing.version;
        let anchor = self.world(self.bounds.get().center());
        let task = cx.read_from_clipboard_async();
        self.read_task = Some(cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                view.read_task = None;
                if !view.can_edit()
                    || view.graph.editing.version != version
                    || !Arc::ptr_eq(&view.graph.projection, &projection)
                {
                    return;
                }
                match result.ok().flatten().and_then(|item| item.text()) {
                    Some(source) => {
                        view.submit(GraphCommand::Paste { source, anchor }, Some(version), cx)
                    }
                    None => view
                        .notifications
                        .error(yss_ui::text::translate("native.canvas.commandFailed"), cx),
                }
                cx.notify();
            })
            .log_released();
        }));
        cx.notify();
    }
}
