//! Node actions reuse the same selection, clipboard, edit and execution entry points.
use crate::canvas::{Gesture, GraphCanvas, GraphCommand, commands::*};
use gpui::{Action, Context, MouseDownEvent, Pixels, Point, Window, div, prelude::*};
use gpui_component::{ActiveTheme, Icon, input, menu::PopupMenuItem};
use gpui_kit_assets::IconName;
use yss_graph_document::{GraphResourceKind, NodeId};
use yss_graph_editor::EditorGraphMutation;

impl GraphCanvas {
    pub(in crate::canvas) fn begin_node_menu(
        &mut self,
        id: NodeId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if self.busy {
            return;
        }
        self.cancel_gesture();
        self.palette = None;
        self.connection_click = None;
        window.focus(&self.focus, cx);
        self.gesture = Some(Gesture::Pan {
            press: event.position,
            previous: event.position,
            moved: false,
            node: Some(id),
        });
        cx.notify();
    }

    pub(in crate::canvas) fn show_node_menu(
        &mut self,
        id: NodeId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let projection = self.graph.projection.clone();
        let version = self.graph.editing.version;
        let language = crate::text::locale();
        let owner = cx.entity().downgrade();
        self.show_context_menu(position, window, cx, move |mut menu, _, cx| {
            let menu_id = cx.entity_id();
            for (index, (command, enabled)) in [
                (NodeAction::Run, run),
                (NodeAction::RunTo, run),
                (NodeAction::Copy, modify),
                (NodeAction::Cut, modify),
                (NodeAction::Duplicate, modify),
                (NodeAction::Disconnect, links),
                (NodeAction::SelectLinked, links),
                (NodeAction::Delete, modify),
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
                let item = if matches!(command, NodeAction::Delete) {
                    PopupMenuItem::element(move |_, cx| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(if enabled {
                                cx.theme().danger
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(Icon::new(icon).size_4())
                            .child(div().flex_1().child(crate::text::t(label)))
                            .child("Del")
                    })
                } else {
                    PopupMenuItem::new(crate::text::t(label)).icon(icon)
                }
                .disabled(!enabled)
                .when_some(action, |item, action| item.action(action));
                let owner = owner.clone();
                let projection = projection.clone();
                menu = menu.item(item.on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |view, cx| {
                        if enabled
                            && view.menu_is_current(menu_id, &projection, version, language)
                            && view.selected.len() == 1
                            && view.selected.contains(&id)
                        {
                            command.invoke(view, id, cx);
                        }
                    });
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
            Self::Copy => view.copy_selection(false, cx),
            Self::Cut => view.copy_selection(true, cx),
            Self::Duplicate => view.duplicate_selection(cx),
            Self::Disconnect => view.submit(
                GraphCommand::Edit(EditorGraphMutation::DisconnectNode { node_id: id }),
                None,
                cx,
            ),
            Self::SelectLinked => view.select_linked_nodes(id, cx),
            Self::Delete => view.delete_selection(cx),
        }
    }
}
