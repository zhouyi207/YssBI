//! Pointer gestures belong to one canvas; only a completed node drag edits the graph.
mod update;

use std::collections::BTreeSet;
use yss_ui::lifecycle::LogReleased;

use gpui_kit::{
    Bounds, Context, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, WeakEntity, Window, point, px,
};
use yss_graph_document::{ConnectionId, NodeId, PortAddress};
use yss_graph_editor::projection::ConnectionDecision;
use yss_project::GraphEditVersion;

use super::{GraphCanvas, connections, geometry};

/// What the pointer is over, front to back: pins and the status lane before their node,
/// nodes before the connections running beneath them.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum CanvasTarget {
    Port(PortAddress),
    Status(NodeId),
    Node(NodeId),
    Connection(ConnectionId),
}

/// How far a dragged wire reaches for a compatible pin, in screen pixels.
const SNAP_DISTANCE: f32 = 20.;

pub(super) enum Gesture {
    Pan {
        press: Point<Pixels>,
        offset: Point<Pixels>,
        button: MouseButton,
        moved: bool,
        node: Option<NodeId>,
    },
    Nodes {
        press: Point<Pixels>,
        version: GraphEditVersion,
        reveal: Option<NodeId>,
        moved: bool,
    },
    Selection {
        press: Point<Pixels>,
        current: Point<Pixels>,
        previous: BTreeSet<NodeId>,
        previous_connections: BTreeSet<ConnectionId>,
        additive: bool,
    },
    Connection(connections::ConnectionDrag),
}

impl Gesture {
    fn button(&self) -> MouseButton {
        match self {
            Self::Pan { button, .. } => *button,
            _ => MouseButton::Left,
        }
    }
}

impl GraphCanvas {
    pub(super) fn cancel_gesture(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Pan { offset, .. }) => self.offset = offset,
            Some(Gesture::Selection {
                previous,
                previous_connections,
                ..
            }) => {
                self.selected = previous;
                self.selected_connections = previous_connections;
            }
            _ => {}
        }
        if !self.busy {
            self.preview.clear();
        }
        self.hover = None;
        self.context_menu = None;
        self.read_task = None;
    }

    pub(super) fn begin_node(
        &mut self,
        id: NodeId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !self.can_edit() {
            return;
        }
        if event.modifiers.alt {
            self.begin_pan(None, event, window, cx);
            return;
        }
        self.cancel_gesture();
        self.located_port = None;
        self.selected_connections.clear();
        self.connection_click = None;
        window.focus(&self.focus, cx);
        self.palette = None;
        let additive = event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
        if additive {
            if !self.selected.insert(id) {
                self.selected.remove(&id);
            }
        } else if !self.selected.contains(&id) {
            self.selected = BTreeSet::from([id]);
        }
        let draggable = self
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.node_id == id)
            .is_some_and(|node| !node.capabilities.managed);
        let positions = self
            .graph
            .projection
            .nodes
            .iter()
            .filter(|node| {
                draggable && !node.capabilities.managed && self.selected.contains(&node.node_id)
            })
            .map(|node| (node.node_id, node.position))
            .collect();
        self.preview = geometry::NodeMovement::new(positions);
        self.gesture = Some(Gesture::Nodes {
            press: event.position,
            version: self.graph.editing.version,
            reveal: (!additive).then_some(id),
            moved: false,
        });
        self.emit_selection(cx);
        cx.notify();
    }

    pub(super) fn begin_pan(
        &mut self,
        node: Option<NodeId>,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !self.can_edit() {
            return;
        }
        self.cancel_gesture();
        self.palette = None;
        self.connection_click = None;
        window.focus(&self.focus, cx);
        self.gesture = Some(Gesture::Pan {
            press: event.position,
            offset: self.offset,
            button: event.button,
            moved: false,
            node,
        });
        cx.notify();
    }

    pub(super) fn begin_pane(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !self.can_edit() {
            return;
        }
        if event.button == MouseButton::Middle {
            self.begin_pan(None, event, window, cx);
            return;
        }
        match self.target_at(event.position) {
            Some(CanvasTarget::Port(address)) => {
                if event.button == MouseButton::Right {
                    self.show_port_menu(address, event.position, window, cx);
                } else {
                    self.begin_port(address, event, window, cx);
                }
                return;
            }
            Some(CanvasTarget::Node(id) | CanvasTarget::Status(id)) => {
                if event.button == MouseButton::Right {
                    self.begin_node_menu(id, event, window, cx);
                } else {
                    self.begin_node(id, event, window, cx);
                }
                return;
            }
            Some(CanvasTarget::Connection(id)) if !event.modifiers.alt => {
                self.begin_connection(id, event, window, cx);
                return;
            }
            _ => {}
        }
        if event.button == MouseButton::Right || event.modifiers.alt {
            self.begin_pan(None, event, window, cx);
            return;
        }
        self.cancel_gesture();
        self.located_port = None;
        self.connection_click = None;
        window.focus(&self.focus, cx);
        self.palette = None;
        let previous = self.selected.clone();
        let previous_connections = self.selected_connections.clone();
        let additive = event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
        if !additive {
            self.selected.clear();
            self.selected_connections.clear();
        }
        self.gesture = Some(Gesture::Selection {
            press: event.position,
            current: event.position,
            previous,
            previous_connections,
            additive,
        });
        self.emit_selection(cx);
        cx.notify();
    }

    /// Register during painting so handlers disappear with the visible canvas.
    /// A window listener keeps the original drag alive outside the canvas and over child inputs.
    pub(super) fn register_gesture_events(owner: WeakEntity<Self>, window: &mut Window) {
        let moving = owner.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            moving
                .update(cx, |view, cx| {
                    if view.gesture.is_some() {
                        view.pointer_move(event, cx);
                    }
                })
                .log_released();
        });
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            owner
                .update(cx, |view, cx| {
                    if view
                        .gesture
                        .as_ref()
                        .is_some_and(|g| g.button() == event.button)
                    {
                        let event = event.clone();
                        // Let a port consume the release first. The deferred fallback also runs
                        // when another child stops bubbling, without retaining window listeners.
                        cx.defer_in(window, move |view, window, cx| {
                            view.pointer_up(&event, window, cx);
                        });
                    }
                })
                .log_released();
        });
    }

    pub(super) fn node_at(&self, screen: Point<Pixels>) -> Option<NodeId> {
        let canvas = self.bounds.get().origin;
        self.painted_nodes()
            .rev()
            .find(|(node, geometry)| {
                self.node_screen_rect(node, geometry, canvas)
                    .contains(&screen)
            })
            .map(|(node, _)| node.node_id)
    }

    /// The region that grabs `address`, relative to `canvas`.
    pub(super) fn port_screen_bounds(
        &self,
        address: &PortAddress,
        canvas: Point<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let (node, geometry) = self
            .painted_nodes()
            .find(|(node, _)| node.node_id == address.node_id)?;
        let index = node
            .ports
            .iter()
            .position(|port| port.address == *address)?;
        let rect = self.node_screen_rect(node, geometry, canvas);
        let label = if self.detail().labels > 0. && !geometry.layout.compact {
            self.node_texts
                .port_label_width(node.node_id, index)
                .unwrap_or(0.)
        } else {
            0.
        };
        [(false, &geometry.inputs), (true, &geometry.outputs)]
            .into_iter()
            .find_map(|(output, indices)| {
                let row = indices.iter().position(|candidate| *candidate == index)?;
                Some(self.port_bounds(rect, geometry.layout, (output, row), label))
            })
    }

    /// The screen region that grabs a port: its pin, reaching past the card edge, and its label.
    fn port_bounds(
        &self,
        rect: Bounds<Pixels>,
        layout: geometry::NodeLayout,
        (output, row): (bool, usize),
        label: f32,
    ) -> Bounds<Pixels> {
        let zoom = self.zoom;
        let region = layout.port_region(output, row);
        let top = rect.top() + region.top() * zoom;
        let bottom = rect.top() + region.bottom() * zoom;
        let reach = px(geometry::PORT_REACH * zoom);
        let inward = px((geometry::CONTENT_INSET + label + 4.) * zoom).min(rect.size.width / 2.);
        if output {
            Bounds::from_corners(
                point(rect.right() - inward, top),
                point(rect.right() + reach, bottom),
            )
        } else {
            Bounds::from_corners(
                point(rect.left() - reach, top),
                point(rect.left() + inward, bottom),
            )
        }
    }

    pub(super) fn target_at(&self, screen: Point<Pixels>) -> Option<CanvasTarget> {
        let canvas = self.bounds.get();
        if !canvas.contains(&screen) {
            return None;
        }
        let reach = px(geometry::PORT_REACH * self.zoom);
        let labels = self.detail().labels > 0.;
        for (node, geometry) in self.painted_nodes().rev() {
            let rect = self.node_screen_rect(node, geometry, canvas.origin);
            if !rect.dilate(reach).contains(&screen) {
                continue;
            }
            for (output, indices) in [(false, &geometry.inputs), (true, &geometry.outputs)] {
                for (row, index) in indices.iter().enumerate() {
                    let Some(port) = node.ports.get(*index) else {
                        continue;
                    };
                    let label = if labels && !geometry.layout.compact {
                        self.node_texts
                            .port_label_width(node.node_id, *index)
                            .unwrap_or(0.)
                    } else {
                        0.
                    };
                    if self
                        .port_bounds(rect, geometry.layout, (output, row), label)
                        .contains(&screen)
                    {
                        return Some(CanvasTarget::Port(port.address.clone()));
                    }
                }
            }
            if !rect.contains(&screen) {
                continue;
            }
            if !geometry.layout.compact && self.status_bounds(rect).contains(&screen) {
                return Some(CanvasTarget::Status(node.node_id));
            }
            return Some(CanvasTarget::Node(node.node_id));
        }
        self.connection_layer
            .hit_test(
                screen - canvas.origin - self.offset,
                self.zoom,
                &self.preview,
            )
            .map(CanvasTarget::Connection)
    }

    /// The pin a dragged wire attaches to: the one under the pointer, or else the nearest
    /// pin the connection could use.
    pub(super) fn snap_target(&self, screen: Point<Pixels>) -> Option<PortAddress> {
        if let Some(CanvasTarget::Port(address)) = self.target_at(screen) {
            return Some(address);
        }
        let drag = self.connection_drag()?;
        let canvas = self.bounds.get();
        let reach = px(SNAP_DISTANCE);
        let mut nearest = None::<(f32, &PortAddress)>;
        for (node, geometry) in self.painted_nodes() {
            let rect = self.node_screen_rect(node, geometry, canvas.origin);
            if !rect.dilate(reach * 2.).contains(&screen) {
                continue;
            }
            for port in node.ports.iter() {
                if !matches!(
                    drag.decision(&port.address),
                    Some(ConnectionDecision::Append | ConnectionDecision::Replace { .. })
                ) {
                    continue;
                }
                let Some(anchor) = self
                    .connection_layer
                    .port_position(&port.address, &self.preview)
                else {
                    continue;
                };
                let anchor = canvas.origin + self.offset + anchor * self.zoom;
                let distance = (anchor - screen).magnitude() as f32;
                if distance <= SNAP_DISTANCE
                    && nearest.is_none_or(|(closest, _)| distance < closest)
                {
                    nearest = Some((distance, &port.address));
                }
            }
        }
        nearest.map(|(_, address)| address.clone())
    }

    pub(super) fn hovered_port(&self) -> Option<&PortAddress> {
        match &self.hover {
            Some(CanvasTarget::Port(address)) => Some(address),
            _ => None,
        }
    }

    pub(super) fn hovered_node(&self) -> Option<NodeId> {
        match &self.hover {
            Some(CanvasTarget::Node(id) | CanvasTarget::Status(id)) => Some(*id),
            Some(CanvasTarget::Port(address)) => Some(address.node_id),
            _ => None,
        }
    }

    pub(super) fn hovered_connection(&self) -> Option<ConnectionId> {
        match &self.hover {
            Some(CanvasTarget::Connection(id)) => Some(*id),
            _ => None,
        }
    }

    pub(super) fn update_hover(&mut self, screen: Point<Pixels>, cx: &mut Context<Self>) {
        let hover = if !self.can_edit() || self.context_menu.is_some() {
            None
        } else {
            self.target_at(screen)
        };
        if hover != self.hover {
            self.hover = hover;
            cx.notify();
        }
    }
}
