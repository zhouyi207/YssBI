//! Measured rendering bounds; DockArea remains the owner of all layout state.
use super::super::{
    Workbench,
    layout::columns,
    status_bar::{ColumnStatusBar, STATUS_BAR_HEIGHT, StatusColumn},
};
use gpui_kit::component::dock::{DockArea, NodeId};
use gpui_kit::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, WeakEntity, Window, div, prelude::*, px,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use yss_ui::lifecycle::LogReleased;

#[derive(Default)]
pub(in crate::workbench) struct Measurements {
    center: Cell<Bounds<Pixels>>,
    editor_left: Cell<Pixels>,
    conversation: RefCell<Vec<(NodeId, AnyElement, Bounds<Pixels>)>>,
}

impl Measurements {
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
            owner.update(cx, |_, cx| cx.notify()).log_released();
        });
    }
}

enum Region {
    Conversation(NodeId),
    ConversationSurface,
    Bottom,
    Status(StatusColumn, Pixels),
}

/// Reuses the original panel element at its measured workbench boundary.
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

pub(super) fn conversation_surface(
    area: WeakEntity<DockArea>,
    owner: WeakEntity<Workbench>,
    measurements: Rc<Measurements>,
) -> impl IntoElement {
    div().absolute().inset_0().child(RegionContent {
        child: None,
        region: Region::ConversationSurface,
        area,
        owner,
        measurements,
    })
}

pub(super) fn status_bars(
    area: WeakEntity<DockArea>,
    owner: WeakEntity<Workbench>,
    measurements: Rc<Measurements>,
    bottom: Pixels,
) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .children(
            [StatusColumn::Conversation, StatusColumn::Editor].map(|column| {
                div().absolute().inset_0().child(RegionContent {
                    child: Some(ColumnStatusBar::new(owner.clone(), column).into_any_element()),
                    region: Region::Status(column, bottom),
                    area: area.clone(),
                    owner: owner.clone(),
                    measurements: measurements.clone(),
                })
            }),
        )
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
        if matches!(self.region, Region::ConversationSurface) {
            let mut surface = {
                let mut conversation = self.measurements.conversation.borrow_mut();
                if conversation.is_empty() {
                    return;
                }
                div()
                    .size_full()
                    .children(conversation.drain(..).map(|(node, child, target)| {
                        div()
                            .id(("conversation-surface", node.as_u64()))
                            .absolute()
                            .left(target.left() - bounds.left())
                            .top(target.top() - bounds.top())
                            .w(target.size.width)
                            .h(target.size.height)
                            .overflow_hidden()
                            .child(child)
                    }))
                    .into_any_element()
            };
            surface.prepaint_as_root(
                bounds.origin,
                bounds.size.map(AvailableSpace::Definite),
                window,
                cx,
            );
            self.child = Some(surface);
            return;
        }
        let mut target = bounds;
        if let Some(area) = self.area.upgrade() {
            let area = area.read(cx);
            match self.region {
                Region::Conversation(node) if columns::full_height_conversation(area, node) => {
                    let blocked = self.owner.upgrade().is_none_or(|owner| {
                        let owner = owner.read(cx);
                        owner.busy || owner.closing
                    });
                    if !blocked {
                        target.size.height =
                            (area.bounds().bottom() - STATUS_BAR_HEIGHT - bounds.top()).max(px(0.));
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
                Region::Status(column, bottom) => {
                    if area.is_zoomed() {
                        self.child = None;
                        return;
                    }
                    // This sibling follows DockArea's prepaint, so the split boundary
                    // is current without deferring chrome above Root's overlays.
                    target = self.measurements.center.get();
                    target.origin.y = target.bottom() - bottom - STATUS_BAR_HEIGHT;
                    target.size.height = STATUS_BAR_HEIGHT;
                    let edge = if columns::conversation_edge(area).is_some() {
                        self.measurements
                            .editor_left
                            .get()
                            .clamp(target.left(), target.right())
                    } else {
                        target.left()
                    };
                    if matches!(column, StatusColumn::Conversation) {
                        target.size.width = edge - target.left();
                    } else {
                        target.size.width = target.right() - edge;
                        target.origin.x = edge;
                    }
                }
                _ => {}
            }
        }
        if target.size.width <= px(0.) || target.size.height <= px(0.) {
            self.child = None;
            return;
        }
        if let Region::Conversation(node) = self.region {
            // Escape the split's clip as an ordinary workbench sibling, not a
            // deferred draw that would also escape above window-level overlays.
            if let Some(child) = self.child.take() {
                self.measurements
                    .conversation
                    .borrow_mut()
                    .push((node, child, target));
            }
        } else if let Some(child) = self.child.as_mut() {
            child.prepaint_as_root(
                target.origin,
                target.size.map(AvailableSpace::Definite),
                window,
                cx,
            );
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
