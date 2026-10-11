//! Shared empty editor presentation, including the editor space beside conversations.
use crate::text;
use gpui_kit::component::{
    ActiveTheme,
    dock::{BasePanel, DockArea, DockPlacement, NodeId, Panel, PanelEvent},
};
use gpui_kit::{
    App, Context, Empty, EventEmitter, FocusHandle, Focusable, FontWeight, IntoElement, Render,
    RenderOnce, WeakEntity, Window, div, prelude::*, px, rems,
};

#[derive(IntoElement)]
pub(super) struct Watermark {
    pub area: WeakEntity<DockArea>,
    pub node: NodeId,
}

impl RenderOnce for Watermark {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // RenderOnce runs after DockArea's render borrow ends, so this reads the
        // current layout directly instead of caching another empty-state flag.
        let visible = self.area.upgrade().is_some_and(|area| {
            let area = area.read(cx);
            area.is_empty(DockPlacement::Center, cx)
                && area
                    .layout(DockPlacement::Center)
                    .is_some_and(|tree| tree.root().id() == self.node)
        });
        if !visible {
            return Empty.into_any_element();
        }

        div()
            .absolute()
            .inset_0()
            .child(content(cx))
            .into_any_element()
    }
}

// DockArea removes empty split groups. This panel keeps the editor column available
// while only conversations are open; it is removed as soon as a resource occupies it.
pub(in crate::workbench) struct EmptyEditor {
    focus: FocusHandle,
}

impl EmptyEditor {
    pub(in crate::workbench) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }
}

impl EventEmitter<PanelEvent> for EmptyEditor {}

impl Focusable for EmptyEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl BasePanel for EmptyEditor {
    fn panel_name(&self) -> &'static str {
        "empty-editor"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for EmptyEditor {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
    fn title_bar(&self, _: &App) -> bool {
        false
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}

impl Render for EmptyEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .child(content(cx))
    }
}

fn content(cx: &App) -> impl IntoElement + use<> {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(cx.theme().table)
        .px_6()
        .py_10()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .child(
                    div()
                        .flex()
                        .text_size(rems(24. / 7.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground.opacity(0.8))
                        .child("Yss")
                        .child(div().text_color(cx.theme().primary).child("BI")),
                )
                .child(
                    div()
                        .my_5()
                        .h(px(1.))
                        .w_16()
                        .bg(cx.theme().primary.opacity(0.45)),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(text::translate("aboutModal.description")),
                ),
        )
}
