//! Measured rendering bounds; DockArea remains the owner of all layout state.
use super::super::{
    Workbench,
    layout::columns,
    status_bar::{ColumnStatusBar, STATUS_BAR_HEIGHT, StatusColumn},
};
use gpui_kit::component::dock::{DockArea, NodeId};
use gpui_kit::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, WeakEntity, Window, deferred, div,
    prelude::*, px,
};
use std::{cell::Cell, rc::Rc};
use yss_ui::lifecycle::LogReleased;

#[derive(Default)]
pub(in crate::workbench) struct Measurements {
    center: Cell<Bounds<Pixels>>,
    editor_left: Cell<Pixels>,
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
    Bottom,
    Status(StatusColumn),
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

pub(super) fn status_bars(
    area: WeakEntity<DockArea>,
    owner: WeakEntity<Workbench>,
    measurements: Rc<Measurements>,
    bottom: Pixels,
) -> impl IntoElement {
    div()
        .absolute()
        .bottom(bottom)
        .left_0()
        .right_0()
        .h(STATUS_BAR_HEIGHT)
        .children(
            [StatusColumn::Conversation, StatusColumn::Editor].map(|column| {
                div().absolute().inset_0().child(deferred(RegionContent {
                    child: Some(ColumnStatusBar::new(owner.clone(), column).into_any_element()),
                    region: Region::Status(column),
                    area: area.clone(),
                    owner: owner.clone(),
                    measurements: measurements.clone(),
                }))
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
                        target.size.height =
                            (area.bounds().bottom() - STATUS_BAR_HEIGHT - bounds.top()).max(px(0.));
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
                Region::Status(column) => {
                    // Resolve after the split's prepaint so footer and tool panel
                    // use the same current-frame column boundary during resizing.
                    let edge = if columns::conversation_edge(area).is_some() {
                        self.measurements
                            .editor_left
                            .get()
                            .clamp(bounds.left(), bounds.right())
                    } else {
                        bounds.left()
                    };
                    if matches!(column, StatusColumn::Conversation) {
                        target.size.width = edge - bounds.left();
                    } else {
                        target.origin.x = edge;
                        target.size.width = bounds.right() - edge;
                    }
                }
                _ => {}
            }
        }
        if target.size.width <= px(0.) || target.size.height <= px(0.) {
            self.child = None;
            return;
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
