//! Native drops use the receiving canvas for coordinates, focus and graph commands.
use super::{CanvasEvent, ConstantDrag, GraphCanvas, GraphCommand};
use crate::workbench::{ActivityDrag, ActivityDrop};
use gpui::{Context, Window};
use yss_graph_editor::EditorGraphMutation;

impl GraphCanvas {
    pub(super) fn drop_activity(
        &mut self,
        drag: &ActivityDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit() {
            return;
        }
        match drag.resolve(&self.graph.project, self.path(), cx) {
            Some(ActivityDrop::OpenGraph(path)) => {
                cx.emit(CanvasEvent::OpenGraph(path.to_owned()));
            }
            Some(ActivityDrop::CreateNode(creation)) => {
                self.focus_drop(window, cx);
                self.create_node_at(creation.clone(), self.world(window.mouse_position()), cx);
            }
            None => {}
        }
    }

    pub(super) fn drop_constant(
        &mut self,
        drag: &ConstantDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_edit()
            || self.graph.project != drag.project
            || self.graph.projection.graph_path != drag.path
            || self.graph.editing.version != drag.version
        {
            return;
        }
        self.focus_drop(window, cx);
        self.submit(
            GraphCommand::Edit(EditorGraphMutation::InsertConstantReference {
                id: drag.id,
                position: self.world(window.mouse_position()),
            }),
            Some(drag.version),
            cx,
        );
    }

    fn focus_drop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_gesture();
        window.focus(&self.focus, cx);
        self.emit_selection(cx);
    }
}
