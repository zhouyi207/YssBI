//! Graph and diagnostic navigation change only canvas selection and viewport state.
use super::{GraphCanvas, geometry};
use gpui_kit::{Bounds, Context, Window, point, px};
use std::collections::BTreeSet;
use yss_graph_document::{ConnectionId, NodeId, PortAddress};

impl GraphCanvas {
    pub(crate) fn reveal_node(
        &mut self,
        id: NodeId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .graph
            .projection
            .nodes
            .iter()
            .any(|node| node.node_id == id)
        {
            return false;
        }
        self.cancel_gesture();
        self.palette = None;
        self.located_port = None;
        self.selected_connections.clear();
        self.connection_click = None;
        self.selected = [id].into();
        self.frame_nodes(Some(&[id].into()), cx);
        window.focus(&self.focus, cx);
        self.emit_selection(cx);
        cx.notify();
        true
    }

    pub(crate) fn reveal_port(
        &mut self,
        address: &PortAddress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .graph
            .projection
            .nodes
            .iter()
            .any(|node| node.ports.iter().any(|port| port.address == *address))
        {
            return false;
        }
        self.reveal_node(address.node_id, window, cx);
        self.located_port = Some(address.clone());
        window.focus(&self.port_focus, cx);
        cx.notify();
        true
    }

    pub(crate) fn reveal_connection(
        &mut self,
        id: ConnectionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(connection) = self
            .graph
            .projection
            .connections
            .iter()
            .find(|connection| connection.connection_id == id)
        else {
            return false;
        };
        let nodes = [connection.output.node_id, connection.input.node_id];
        if !nodes.iter().all(|id| {
            self.graph
                .projection
                .nodes
                .iter()
                .any(|node| node.node_id == *id)
        }) {
            return false;
        }
        self.cancel_gesture();
        self.palette = None;
        self.selected.clear();
        self.located_port = None;
        self.selected_connections = [id].into();
        self.connection_click = None;
        self.frame_nodes(Some(&nodes.into()), cx);
        window.focus(&self.focus, cx);
        self.emit_selection(cx);
        cx.notify();
        true
    }

    pub(crate) fn reveal_graph(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_gesture();
        self.palette = None;
        self.frame_nodes(None, cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub(super) fn frame_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        self.cancel_gesture();
        self.palette = None;
        self.frame_nodes(Some(&self.selected.clone()), cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn frame_nodes(&mut self, ids: Option<&BTreeSet<NodeId>>, cx: &mut Context<Self>) {
        let bounds = self
            .graph
            .projection
            .nodes
            .iter()
            .filter(|node| ids.is_none_or(|ids| ids.contains(&node.node_id)))
            .map(|node| geometry::node_bounds(node, node.position))
            .reduce(|a, b| {
                Bounds::from_corners(
                    point(a.left().min(b.left()), a.top().min(b.top())),
                    point(a.right().max(b.right()), a.bottom().max(b.bottom())),
                )
            });
        let Some(bounds) = bounds else { return };
        let viewport = self.bounds.get().size;
        if viewport.width <= px(0.) || viewport.height <= px(0.) {
            return;
        }
        let width = f32::from((viewport.width - px(128. * self.font_scale)).max(px(1.)));
        let height = f32::from((viewport.height - px(128. * self.font_scale)).max(px(1.)));
        self.zoom = (width / f32::from(bounds.size.width))
            .min(height / f32::from(bounds.size.height))
            .clamp(
                crate::services::Viewport::MIN_SCALE * self.font_scale,
                crate::services::Viewport::MAX_SCALE * self.font_scale,
            );
        self.offset = point(
            (viewport.width - bounds.size.width * self.zoom) / 2. - bounds.origin.x * self.zoom,
            (viewport.height - bounds.size.height * self.zoom) / 2. - bounds.origin.y * self.zoom,
        );
        self.checkpoint_viewport(cx);
    }

    pub(super) fn retain_located(&mut self) {
        self.retain_connections();
        if self.located_port.as_ref().is_some_and(|address| {
            !self
                .graph
                .projection
                .nodes
                .iter()
                .any(|node| node.ports.iter().any(|port| port.address == *address))
        }) {
            self.located_port = None;
        }
    }
}
