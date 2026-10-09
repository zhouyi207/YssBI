//! Diagnostic navigation changes only canvas selection and viewport state.
use super::{GraphCanvas, geometry};
use gpui::{Bounds, Context, Window, point, px};
use yss_graph_document::{ConnectionId, NodeId, PortAddress};

#[derive(Clone, PartialEq, Eq)]
pub(super) enum LocatedElement {
    Port(PortAddress),
    Connection(ConnectionId),
}

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
        self.located = None;
        self.selected = [id].into();
        self.frame_nodes(Some(&[id]));
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
        self.located = Some(LocatedElement::Port(address.clone()));
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
        self.located = Some(LocatedElement::Connection(id));
        self.frame_nodes(Some(&nodes));
        window.focus(&self.focus, cx);
        self.emit_selection(cx);
        cx.notify();
        true
    }

    pub(crate) fn reveal_graph(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_gesture();
        self.palette = None;
        self.frame_nodes(None);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn frame_nodes(&mut self, ids: Option<&[NodeId]>) {
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
        let width = f32::from((viewport.width - px(80.)).max(px(1.)));
        let height = f32::from((viewport.height - px(80.)).max(px(1.)));
        self.zoom = (width / f32::from(bounds.size.width))
            .min(height / f32::from(bounds.size.height))
            .clamp(0.1, 1.);
        self.offset = point(
            (viewport.width - bounds.size.width * self.zoom) / 2. - bounds.origin.x * self.zoom,
            (viewport.height - bounds.size.height * self.zoom) / 2. - bounds.origin.y * self.zoom,
        );
    }

    pub(super) fn retain_located(&mut self) {
        let valid = match &self.located {
            Some(LocatedElement::Port(address)) => self
                .graph
                .projection
                .nodes
                .iter()
                .any(|node| node.ports.iter().any(|port| port.address == *address)),
            Some(LocatedElement::Connection(id)) => self
                .graph
                .projection
                .connections
                .iter()
                .any(|connection| connection.connection_id == *id),
            None => true,
        };
        if !valid {
            self.located = None;
        }
    }
}
