//! Connection selection and commands belong to the same canvas as node gestures.
use crate::canvas::{GraphCanvas, GraphCommand};
use gpui::{Context, MouseButton, MouseDownEvent, Pixels, Point, Window};
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
    pub(super) fn hit_connection(&mut self, screen: Point<Pixels>) -> Option<ConnectionId> {
        let bounds = self.bounds.get();
        if !bounds.contains(&screen) {
            return None;
        }
        let local = screen - bounds.origin - self.offset;
        // Nodes cover the line layer, including right presses that bubble from their body.
        if self.node_at(screen).is_some() {
            return None;
        }
        self.connection_layer
            .borrow_mut()
            .hit_test(local, self.zoom, &self.preview)
    }

    pub(in crate::canvas) fn begin_connection(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.button == MouseButton::Middle {
            return false;
        }
        let Some(id) = self.hit_connection(event.position) else {
            return false;
        };
        cx.stop_propagation();
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
        true
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

    pub(in crate::canvas) fn hover_connection(
        &mut self,
        screen: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let hovered = if self.busy || self.context_menu.is_some() {
            None
        } else {
            self.hit_connection(screen)
        };
        if hovered != self.hovered_connection {
            self.hovered_connection = hovered;
            cx.notify();
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
        self.hovered_connection = self.hovered_connection.filter(exists);
        self.connection_click = None;
        self.context_menu = None;
    }
}
