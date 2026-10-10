use super::{Gesture, GraphCanvas};
use crate::canvas::{CanvasEvent, GraphCommand, geometry};
use gpui::{
    Context, MouseButton, MouseMoveEvent, MouseUpEvent, PinchEvent, Pixels, Point,
    ScrollWheelEvent, Window, px,
};
use std::collections::BTreeSet;
use yss_graph_document::NodePosition;
use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};

impl GraphCanvas {
    pub(super) fn pointer_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button.is_none() {
            self.cancel_gesture();
            self.emit_selection(cx);
        } else {
            self.update_gesture_position(event.position, cx);
        }
        cx.notify();
    }

    fn update_gesture_position(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(mut gesture) = self.gesture.take() else {
            return;
        };
        match &mut gesture {
            Gesture::Pan {
                press,
                offset,
                moved,
                ..
            } => {
                let delta = position - *press;
                *moved |= delta.magnitude() > 3.;
                self.offset = *offset + delta;
            }
            Gesture::Nodes {
                press,
                positions,
                moved,
                ..
            } => {
                let delta = (position - *press) / self.zoom;
                *moved |= delta.x != px(0.) || delta.y != px(0.);
                self.preview = positions
                    .iter()
                    .filter_map(|(id, p)| {
                        let next = NodePosition {
                            x: p.x + f32::from(delta.x) as f64,
                            y: p.y + f32::from(delta.y) as f64,
                        };
                        (next != *p).then_some((*id, next))
                    })
                    .collect();
            }
            Gesture::Selection {
                press,
                current,
                previous,
                additive,
                ..
            } => {
                if *current == position {
                    self.gesture = Some(gesture);
                    return;
                }
                *current = position;
                let a = self.world(*press);
                let b = self.world(*current);
                let mut selected = self
                    .graph
                    .projection
                    .nodes
                    .iter()
                    .filter(|node| {
                        let rect = geometry::node_bounds(node, node.position);
                        !node.capabilities.managed
                            && rect.right() >= px(a.x.min(b.x) as f32)
                            && rect.left() <= px(a.x.max(b.x) as f32)
                            && rect.bottom() >= px(a.y.min(b.y) as f32)
                            && rect.top() <= px(a.y.max(b.y) as f32)
                    })
                    .map(|node| node.node_id)
                    .collect::<BTreeSet<_>>();
                if *additive {
                    selected.extend(previous.iter().copied());
                }
                if self.selected != selected {
                    self.selected = selected;
                    self.emit_selection(cx);
                }
            }
            Gesture::Connection(drag) => drag.current = position,
        }
        self.gesture = Some(gesture);
    }

    pub(in crate::canvas) fn pointer_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .gesture
            .as_ref()
            .is_none_or(|g| g.button() != event.button)
        {
            return;
        }
        self.update_gesture_position(event.position, cx);
        match self.gesture.take() {
            Some(Gesture::Nodes {
                version,
                reveal,
                moved,
                ..
            }) => {
                let positions = self
                    .preview
                    .iter()
                    .map(|(node_id, position)| NodePositionMutation {
                        node_id: *node_id,
                        position: *position,
                    })
                    .collect::<Vec<_>>();
                if !positions.is_empty() {
                    self.submit(
                        GraphCommand::Edit(EditorGraphMutation::MoveNodes { positions }),
                        Some(version),
                        cx,
                    );
                    if !self.busy {
                        self.preview.clear();
                    }
                } else if let Some(id) = reveal.filter(|_| !moved) {
                    if self.selected.len() != 1 || !self.selected.contains(&id) {
                        self.selected = [id].into();
                        self.emit_selection(cx);
                    }
                    cx.emit(CanvasEvent::RevealNodeDetails);
                }
            }
            Some(Gesture::Pan {
                moved: false, node, ..
            }) if event.button == MouseButton::Right
                && self.bounds.get().contains(&event.position) =>
            {
                if let Some(id) = node.filter(|id| self.node_at(event.position) == Some(*id)) {
                    self.show_node_menu(id, event.position, window, cx);
                } else if node.is_none() && self.node_at(event.position).is_none() {
                    self.show_palette(event.position, None, window, cx);
                }
            }
            Some(Gesture::Connection(drag))
                if !drag.moving
                    && (event.position - drag.press).magnitude() >= 5.
                    && self.bounds.get().contains(&event.position)
                    && self.node_at(event.position).is_none() =>
            {
                self.show_palette(event.position, Some(drag.source), window, cx);
            }
            _ => {}
        }
        self.commit_blurred_port_inputs(window, cx);
        cx.notify();
    }

    fn zoom_at(&mut self, position: Point<Pixels>, factor: f32, cx: &mut Context<Self>) {
        if self.busy || self.gesture.is_some() || !factor.is_finite() || factor <= 0. {
            return;
        }
        let zoom = (self.zoom * factor).clamp(0.1, 5.);
        let pointer = position - self.bounds.get().origin;
        self.offset = pointer - (pointer - self.offset) * (zoom / self.zoom);
        self.zoom = zoom;
        cx.notify();
    }

    pub(in crate::canvas) fn zoom_at_pointer(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
        self.zoom_at(event.position, (delta * 0.002).exp(), cx);
    }

    pub(in crate::canvas) fn pinch(
        &mut self,
        event: &PinchEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.zoom_at(event.position, 1. + event.delta, cx);
    }
}
