//! Selection commands share the canvas projection and the normal edit transaction.
use super::{GraphCanvas, GraphCommand};
use gpui_kit::Context;
use yss_graph_document::{NodeId, NodePosition};
use yss_graph_editor::EditorGraphMutation;

impl GraphCanvas {
    pub(super) fn can_copy_selection(&self) -> bool {
        self.can_edit()
            && !self.selected.is_empty()
            && self
                .graph
                .projection
                .nodes
                .iter()
                .filter(|node| self.selected.contains(&node.node_id) && !node.capabilities.managed)
                .count()
                == self.selected.len()
    }

    pub(super) fn duplicate_selection(&mut self, cx: &mut Context<Self>) {
        if self.can_copy_selection() {
            self.submit(
                GraphCommand::Edit(EditorGraphMutation::DuplicateSubgraph {
                    node_ids: self.selected.iter().copied().collect(),
                    offset: NodePosition { x: 32., y: 32. },
                }),
                None,
                cx,
            );
        }
    }

    pub(super) fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selected_connections.is_empty() {
            self.submit(
                GraphCommand::Edit(EditorGraphMutation::DisconnectConnections {
                    connection_ids: self.selected_connections.iter().copied().collect(),
                }),
                None,
                cx,
            );
            return;
        }
        let node_ids = self
            .graph
            .projection
            .nodes
            .iter()
            .filter(|node| self.selected.contains(&node.node_id) && !node.capabilities.managed)
            .map(|node| node.node_id)
            .collect::<Vec<_>>();
        if !node_ids.is_empty() {
            self.submit(
                GraphCommand::Edit(EditorGraphMutation::DeleteNodes { node_ids }),
                None,
                cx,
            );
        }
    }

    pub(super) fn select_linked_nodes(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.cancel_gesture();
        self.selected = self
            .graph
            .projection
            .connections
            .iter()
            .filter_map(|edge| {
                if edge.output.node_id == id && edge.input.node_id != id {
                    Some(edge.input.node_id)
                } else if edge.input.node_id == id && edge.output.node_id != id {
                    Some(edge.output.node_id)
                } else {
                    None
                }
            })
            .collect();
        self.selected_connections.clear();
        self.located_port = None;
        self.emit_selection(cx);
        cx.notify();
    }
}
