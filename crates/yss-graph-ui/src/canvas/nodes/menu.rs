//! Node actions reuse the same selection, clipboard, edit and execution entry points.
use crate::canvas::{GraphCanvas, GraphCommand, GraphEditCommand, commands::*};
use gpui_kit::assets::IconName;
use gpui_kit::component::{input, menu::PopupMenuItem};
use gpui_kit::{Action, Context, MouseDownEvent, Pixels, Point, Window, prelude::*};
use yss_graph_document::{GraphResourceKind, NodeId};
use yss_graph_editor::EditorGraphMutation;
use yss_ui::lifecycle::LogReleased;

impl GraphCanvas {
    pub(in crate::canvas) fn begin_node_menu(
        &mut self,
        id: NodeId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_pan(Some(id), event, window, cx);
    }

    pub(in crate::canvas) fn show_node_menu(
        &mut self,
        id: NodeId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commit_port_inputs_before(window, cx, move |view, window, cx| {
            view.show_node_menu(id, position, window, cx)
        }) {
            return;
        }
        let Some(node) = self
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.node_id == id)
        else {
            return;
        };
        let modify = !node.capabilities.managed;
        let links = node.ports.iter().any(|port| port.connections.current > 0);
        let event_graph = self.graph.projection.graph_path.kind() == GraphResourceKind::EventGraph;
        let run = self.can_run();
        self.cancel_gesture();
        // The React menu targets the right-clicked node, independently of prior multi-selection.
        self.selected = [id].into();
        self.selected_connections.clear();
        self.located_port = None;
        self.emit_selection(cx);
        let copy = self.is_edit_command_enabled(GraphEditCommand::Copy);
        let cut = self.is_edit_command_enabled(GraphEditCommand::Cut);
        let delete = self.is_edit_command_enabled(GraphEditCommand::Delete);
        let projection = self.graph.projection.clone();
        let version = self.graph.editing.version;
        let language = yss_ui::text::locale();
        let owner = cx.entity().downgrade();
        self.show_context_menu(position, window, cx, move |mut menu, _, cx| {
            let menu_id = cx.entity_id();
            for (index, (command, enabled)) in [
                (NodeAction::Run, run),
                (NodeAction::RunTo, run),
                (NodeAction::Copy, copy),
                (NodeAction::Cut, cut),
                (NodeAction::Duplicate, modify),
                (NodeAction::Disconnect, links),
                (NodeAction::SelectLinked, links),
                (NodeAction::Delete, delete),
            ]
            .into_iter()
            .enumerate()
            {
                if index < 2 && !event_graph {
                    continue;
                }
                if (index == 2 && event_graph) || index == 5 || index == 7 {
                    menu = menu.separator();
                }
                let (label, icon, action) = command.presentation();
                let item = PopupMenuItem::new(yss_ui::text::t(label))
                    .icon(icon)
                    .disabled(!enabled)
                    .when_some(action, |item, action| item.action(action));
                let owner = owner.clone();
                let projection = projection.clone();
                menu = menu.item(item.on_click(move |_, _, cx| {
                    owner
                        .update(cx, |view, cx| {
                            if enabled
                                && view.menu_is_current(menu_id, &projection, version, language)
                                && view.selected.len() == 1
                                && view.selected.contains(&id)
                            {
                                command.invoke(view, id, cx);
                            }
                        })
                        .log_released();
                }));
            }
            menu
        });
    }
}

#[derive(Clone, Copy)]
enum NodeAction {
    Run,
    RunTo,
    Copy,
    Cut,
    Duplicate,
    Disconnect,
    SelectLinked,
    Delete,
}

impl NodeAction {
    fn presentation(self) -> (&'static str, IconName, Option<Box<dyn Action>>) {
        match self {
            Self::Run => (
                "contextMenu.node.runNode",
                IconName::Play,
                Some(Box::new(RunCurrentNode)),
            ),
            Self::RunTo => (
                "contextMenu.node.runTo",
                IconName::Play,
                Some(Box::new(RunToNode)),
            ),
            Self::Copy => (
                "contextMenu.node.copy",
                IconName::Copy,
                Some(Box::new(input::Copy)),
            ),
            Self::Cut => (
                "contextMenu.node.cut",
                IconName::Scissors,
                Some(Box::new(input::Cut)),
            ),
            Self::Duplicate => (
                "contextMenu.node.duplicate",
                IconName::CopyPlus,
                Some(Box::new(DuplicateSelection)),
            ),
            Self::Disconnect => ("contextMenu.node.breakAllLinks", IconName::Unlink, None),
            Self::SelectLinked => ("contextMenu.node.selectLinkedNodes", IconName::Link, None),
            Self::Delete => (
                "contextMenu.node.delete",
                IconName::Trash,
                Some(Box::new(DeleteSelection)),
            ),
        }
    }

    fn invoke(self, view: &mut GraphCanvas, id: NodeId, cx: &mut Context<GraphCanvas>) {
        use yss_graph_execution::plan::NodeExecutionMode;
        match self {
            Self::Run => view.run_selected(NodeExecutionMode::CurrentInputs, cx),
            Self::RunTo => view.run_selected(NodeExecutionMode::Dependencies, cx),
            Self::Copy => view.edit(GraphEditCommand::Copy, cx),
            Self::Cut => view.edit(GraphEditCommand::Cut, cx),
            Self::Duplicate => view.duplicate_selection(cx),
            Self::Disconnect => view.submit(
                GraphCommand::Edit(EditorGraphMutation::DisconnectNode { node_id: id }),
                None,
                cx,
            ),
            Self::SelectLinked => view.select_linked_nodes(id, cx),
            Self::Delete => view.edit(GraphEditCommand::Delete, cx),
        }
    }
}
