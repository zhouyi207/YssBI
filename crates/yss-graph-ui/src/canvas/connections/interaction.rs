//! Connection selection and commands belong to the same canvas as node gestures.
use crate::canvas::{GraphCanvas, GraphCommand};
use gpui_kit::{Context, MouseButton, MouseDownEvent, Window};
use std::{collections::BTreeSet, sync::Arc};
use yss_graph_document::{ConnectionId, NodeId};
use yss_graph_editor::{EditorGraphMutation, projection::EditorProjectionModel};

pub(in crate::canvas) struct ConnectionClick {
    id: ConnectionId,
    projection: Arc<EditorProjectionModel>,
    nodes: BTreeSet<NodeId>,
    connections: BTreeSet<ConnectionId>,
    temporary: BTreeSet<ConnectionId>,
}

impl GraphCanvas {
    pub(in crate::canvas) fn begin_connection(
        &mut self,
        id: ConnectionId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_gesture();
        self.located_port = None;
        self.palette = None;
        self.context_menu = None;
        window.focus(&self.focus, cx);
        if event.button == MouseButton::Right {
            if !self.selected_connections.contains(&id) {
                self.selected_connections = [id].into();
            }
            self.selected.clear();
            self.connection_click = None;
            self.show_connection_menu(event.position, window, cx);
        } else if event.click_count == 2 {
            if self.connection_click.as_ref().is_some_and(|click| {
                click.id != id || !Arc::ptr_eq(&click.projection, &self.graph.projection)
            }) {
                self.connection_click = None;
            }
            self.submit(
                GraphCommand::Edit(EditorGraphMutation::InsertReroute {
                    connection_id: id,
                    position: self.world(event.position),
                }),
                Some(self.graph.editing.version),
                cx,
            );
        } else if event.click_count == 1 {
            let nodes = self.selected.clone();
            let connections = self.selected_connections.clone();
            let toggle =
                event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
            if !toggle {
                self.selected_connections.clear();
            }
            if !self.selected_connections.insert(id) && toggle {
                self.selected_connections.remove(&id);
            }
            self.selected.clear();
            self.connection_click = Some(ConnectionClick {
                id,
                projection: self.graph.projection.clone(),
                nodes,
                connections,
                temporary: self.selected_connections.clone(),
            });
        }
        self.emit_selection(cx);
        cx.notify();
    }

    pub(in crate::canvas) fn restore_connection_click(
        &mut self,
        click: ConnectionClick,
        cx: &mut Context<Self>,
    ) {
        if Arc::ptr_eq(&click.projection, &self.graph.projection)
            && self.selected.is_empty()
            && self.selected_connections == click.temporary
        {
            self.selected = click.nodes;
            self.selected_connections = click.connections;
            self.emit_selection(cx);
        }
    }

    pub(in crate::canvas) fn retain_connections(&mut self) {
        if !self.selected_connections.is_empty() {
            self.selected_connections = self
                .graph
                .projection
                .connections
                .iter()
                .filter_map(|edge| {
                    self.selected_connections
                        .contains(&edge.connection_id)
                        .then_some(edge.connection_id)
                })
                .collect();
        }
        let exists = |id: &ConnectionId| {
            self.graph
                .projection
                .connections
                .iter()
                .any(|edge| edge.connection_id == *id)
        };
        if self
            .hovered_connection()
            .is_some_and(|connection| !exists(&connection))
        {
            self.hover = None;
        }
        self.connection_click = None;
        self.context_menu = None;
    }
}
