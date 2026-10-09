//! Measured rendering bounds; DockArea remains the owner of all layout state.
use super::super::{Workbench, layout::columns};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::dock::{DockArea, NodeId};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub(in crate::workbench) struct Measurements {
    center: Cell<Bounds<Pixels>>,
    editor_left: Cell<Pixels>,
}

impl Measurements {
    pub(in crate::workbench) fn leading_width(&self) -> Pixels {
        (self.editor_left.get() - self.center.get().left()).max(px(0.))
    }

    pub(super) fn record_center(
        &self,
        bounds: Bounds<Pixels>,
        has_conversation: bool,
        owner: &WeakEntity<Workbench>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let previous = self.center.replace(bounds);
        if !has_conversation {
            self.record_edge(bounds.left(), owner, window, cx);
        } else if previous.left() != bounds.left() {
            Self::notify_owner(owner, window, cx);
        }
    }

    pub(super) fn record_edge(
        &self,
        edge: Pixels,
        owner: &WeakEntity<Workbench>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.editor_left.replace(edge) != edge {
            Self::notify_owner(owner, window, cx);
        }
    }

    fn notify_owner(owner: &WeakEntity<Workbench>, window: &mut Window, cx: &mut App) {
        let owner = owner.clone();
        window.defer(cx, move |_, cx| {
            let _ = owner.update(cx, |_, cx| cx.notify());
        });
    }
}

enum Region {
    Conversation(NodeId),
    Bottom,
}

/// Reuses the original panel element at its measured workbench boundary.
/// Only conversation content needs deferred painting to extend past the center split's clip.
pub(super) struct RegionContent {
    child: Option<AnyElement>,
    region: Region,
    area: WeakEntity<DockArea>,
    owner: WeakEntity<Workbench>,
    measurements: Rc<Measurements>,
}

impl RegionContent {
    pub(super) fn conversation(
        child: AnyElement,
        node: NodeId,
        area: WeakEntity<DockArea>,
        owner: WeakEntity<Workbench>,
        measurements: Rc<Measurements>,
    ) -> Self {
        Self {
            child: Some(child),
            region: Region::Conversation(node),
            area,
            owner,
            measurements,
        }
    }

    pub(super) fn bottom(
        child: AnyElement,
        area: WeakEntity<DockArea>,
        owner: WeakEntity<Workbench>,
        measurements: Rc<Measurements>,
    ) -> Self {
        Self {
            child: Some(child),
            region: Region::Bottom,
            area,
            owner,
            measurements,
        }
    }
}

impl IntoElement for RegionContent {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for RegionContent {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut placeholder = div()
            .size_full()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .into_any_element();
        (placeholder.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut target = bounds;
        let mut extend = false;
        if let Some(area) = self.area.upgrade() {
            let area = area.read(cx);
            match self.region {
                Region::Conversation(node) if columns::full_height_conversation(area, node) => {
                    let blocked = self.owner.upgrade().is_none_or(|owner| {
                        let owner = owner.read(cx);
                        owner.busy || owner.closing
                    });
                    if !blocked {
                        target.size.height = (area.bounds().bottom() - bounds.top()).max(px(0.));
                        extend = target.size.height > bounds.size.height;
                    }
                }
                Region::Bottom if columns::conversation_edge(area).is_some() => {
                    let left = self
                        .measurements
                        .editor_left
                        .get()
                        .clamp(bounds.left(), bounds.right());
                    target.origin.x = left;
                    target.size.width = bounds.right() - left;
                }
                _ => {}
            }
        }
        let available = target.size.map(AvailableSpace::Definite);
        if extend {
            let mut child = self.child.take().expect("region content is painted once");
            child.layout_as_root(available, window, cx);
            window.defer_draw(
                child,
                target.origin,
                0,
                Some(ContentMask { bounds: target }),
            );
        } else if let Some(child) = self.child.as_mut() {
            child.prepaint_as_root(target.origin, available, window, cx);
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = self.child.as_mut() {
            child.paint(window, cx);
        }
    }
}
