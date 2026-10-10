use super::{Gesture, MindCanvas, MindEvent};
use gpui_kit::{
    Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    ScrollWheelEvent, Window, point, px, size,
};

impl MindCanvas {
    pub(super) fn cancel_gesture(&mut self) {
        if let Some(gesture) = self.gesture.take() {
            match gesture {
                Gesture::Pan { offset, .. } => self.offset = offset,
                Gesture::Selection { previous, .. } => self.selected = previous,
            }
        }
    }
    pub(super) fn begin_topic(
        &mut self,
        id: String,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.modifiers.alt {
            return;
        }
        self.cancel_gesture();
        let additive = event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
        if additive {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
        } else {
            self.selected.clear();
            self.selected.insert(id);
        }
        self.focus_canvas(window, cx);
        self.changed(cx);
        cx.stop_propagation();
    }
    pub(super) fn begin_pane(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_gesture();
        self.focus_canvas(window, cx);
        if event.button != MouseButton::Left || event.modifiers.alt {
            self.gesture = Some(Gesture::Pan {
                press: event.position,
                offset: self.offset,
            });
        } else {
            let additive =
                event.modifiers.shift || event.modifiers.control || event.modifiers.platform;
            let previous = self.selected.clone();
            if !additive {
                self.selected.clear();
            }
            self.gesture = Some(Gesture::Selection {
                press: event.position,
                current: event.position,
                previous,
                additive,
            });
        }
        self.changed(cx);
    }
    pub(super) fn pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(gesture) = &mut self.gesture else {
            return;
        };
        match gesture {
            Gesture::Pan { press, offset } => self.offset = *offset + event.position - *press,
            Gesture::Selection {
                press,
                current,
                previous,
                additive,
            } => {
                *current = event.position;
                let start = (*press - self.bounds.get().origin - self.offset) / self.zoom;
                let end = (*current - self.bounds.get().origin - self.offset) / self.zoom;
                let rect = rectangle(start, end);
                self.selected = if *additive {
                    previous.clone()
                } else {
                    Default::default()
                };
                for node in &self.layout.nodes {
                    if intersects(rect, node.bounds) {
                        self.selected
                            .insert(self.snapshot.content.nodes[node.index].id.clone());
                    }
                }
            }
        }
        self.changed(cx);
    }
    pub(super) fn pointer_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.gesture = None;
        cx.emit(MindEvent::Changed);
        cx.notify();
    }
    pub(super) fn zoom_at_pointer(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.gesture.is_some() {
            return;
        }
        let delta = f32::from(event.delta.pixel_delta(px(20.)).y);
        let zoom = (self.zoom * (delta * 0.002).exp()).clamp(0.1, 3.);
        let pointer = event.position - self.bounds.get().origin;
        self.offset = pointer - (pointer - self.offset) * (zoom / self.zoom);
        self.zoom = zoom;
        cx.notify();
    }
    pub(super) fn fit(&mut self, selection: bool, cx: &mut Context<Self>) {
        let viewport = self.bounds.get().size;
        if viewport.width <= px(0.) || viewport.height <= px(0.) {
            return;
        }
        let rect = if selection && !self.selected.is_empty() {
            let mut selected = self.layout.nodes.iter().filter(|node| {
                self.selected
                    .contains(&self.snapshot.content.nodes[node.index].id)
            });
            let Some(first) = selected.next() else {
                return;
            };
            selected.fold(first.bounds, |bounds, node| bounds.union(&node.bounds))
        } else {
            self.layout.bounds
        };
        self.zoom = ((f32::from(viewport.width) - 96.) / f32::from(rect.size.width).max(1.))
            .min((f32::from(viewport.height) - 96.) / f32::from(rect.size.height).max(1.))
            .clamp(0.1, 1.2);
        self.offset = point(
            (viewport.width - rect.size.width * self.zoom) / 2.,
            (viewport.height - rect.size.height * self.zoom) / 2.,
        ) - rect.origin * self.zoom;
        self.fit_pending = false;
        cx.notify();
    }
    pub(super) fn selection_bounds(&self) -> Option<Bounds<Pixels>> {
        let Some(Gesture::Selection { press, current, .. }) = &self.gesture else {
            return None;
        };
        Some(rectangle(
            *press - self.bounds.get().origin,
            *current - self.bounds.get().origin,
        ))
    }
}
fn rectangle(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(a.x.min(b.x), a.y.min(b.y)),
        size((a.x - b.x).abs(), (a.y - b.y).abs()),
    )
}
pub(super) fn intersects(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.origin.x <= b.origin.x + b.size.width
        && b.origin.x <= a.origin.x + a.size.width
        && a.origin.y <= b.origin.y + b.size.height
        && b.origin.y <= a.origin.y + a.size.height
}
