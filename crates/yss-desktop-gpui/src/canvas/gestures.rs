//! Pointer gestures belong to one canvas; only a completed node drag edits the graph.
mod update;

use std::collections::{BTreeMap, BTreeSet};

use gpui::{
    Context, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, WeakEntity, Window,
};
use yss_graph_document::{ConnectionId, NodeId, NodePosition};
use yss_project::GraphEditVersion;

use super::{GraphCanvas, connections, geometry};

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
        positions: BTreeMap<NodeId, NodePosition>,
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
        self.hovered_connection = None;
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
        if self.busy {
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
        self.gesture = Some(Gesture::Nodes {
            press: event.position,
            positions,
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
        if self.busy {
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
        if self.busy {
            return;
        }
        if event.button == MouseButton::Middle || event.modifiers.alt {
            self.begin_pan(None, event, window, cx);
            return;
        }
        if self.begin_connection(event, window, cx) {
            return;
        }
        if event.button == MouseButton::Right {
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
            let _ = moving.update(cx, |view, cx| {
                if view.gesture.is_some() {
                    view.pointer_move(event, cx);
                }
            });
        });
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            let _ = owner.update(cx, |view, cx| {
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
            });
        });
    }

    pub(super) fn node_at(&self, screen: Point<Pixels>) -> Option<NodeId> {
        let world = (screen - self.bounds.get().origin - self.offset) / self.zoom;
        self.graph.projection.nodes.iter().rev().find_map(|node| {
            geometry::node_bounds(node, geometry::position(node, &self.preview))
                .contains(&world)
                .then_some(node.node_id)
        })
    }
}
