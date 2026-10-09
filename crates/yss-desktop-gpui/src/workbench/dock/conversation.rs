//! A conversation group has one toolbar; the sidebar selects its active session.
use super::super::{Workbench, layout::columns};
use crate::assistant::ConversationPanel;
use gpui::{AnyElement, App, Empty, Entity, IntoElement, WeakEntity, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    dock::{ClosePanel, DockArea, TabGroupContext},
    menu::{DropdownMenu, PopupMenuItem},
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;

pub(super) fn matches(group: &TabGroupContext) -> bool {
    !group.panels().is_empty() && group.panels().iter().all(columns::is_conversation)
}

fn close(workbench: &WeakEntity<Workbench>, window: &mut Window, cx: &mut App) {
    let _ = workbench.update(cx, |view, cx| view.close_conversation_window(window, cx));
}

pub(super) fn render(
    group: &TabGroupContext,
    workbench: &WeakEntity<Workbench>,
    area: &WeakEntity<DockArea>,
    cx: &mut App,
) -> AnyElement {
    let Some(panel) = group
        .active_panel()
        .and_then(|panel| panel.view().downcast::<ConversationPanel>().ok())
    else {
        return Empty.into_any_element();
    };
    let title = panel.read(cx).display_title();
    let mode = panel.update(cx, |panel, cx| panel.read_only_button(cx));
    let locked = area.upgrade().is_none_or(|area| area.read(cx).is_locked());
    let busy = workbench
        .upgrade()
        .is_none_or(|owner| owner.read(cx).assistant_busy || owner.read(cx).is_closing(cx));
    div()
        .id("conversation-column-header")
        .h(px(36.))
        .flex_shrink_0()
        .min_w_0()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .bg(cx.theme().tab_bar)
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Icon::new(IconName::MessageSquareText)
                .size_4()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .id("conversation-title")
                .flex_1()
                .min_w_0()
                .px_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .tooltip({
                    let title = title.clone();
                    move |window, cx| Tooltip::new(title.clone()).build(window, cx)
                })
                .child(title),
        )
        .child(mode)
        .child(
            Button::new("conversation-new")
                .xsmall()
                .ghost()
                .icon(IconName::Plus)
                .tooltip(crate::text::t("native.workbench.newConversation"))
                .disabled(busy)
                .on_click({
                    let workbench = workbench.clone();
                    move |_, window, cx| {
                        let _ = workbench.update(cx, |view, cx| view.new_conversation(window, cx));
                    }
                }),
        )
        .child(
            Button::new("conversation-zoom")
                .xsmall()
                .ghost()
                .icon(if group.is_zoomed() {
                    IconName::Minimize
                } else {
                    IconName::Maximize
                })
                .selected(group.is_zoomed())
                .disabled(locked)
                .tooltip(if group.is_zoomed() {
                    crate::text::t("native.workbench.restoreLayout")
                } else {
                    crate::text::t("native.workbench.expandConversation")
                })
                .on_click({
                    let group = group.clone();
                    move |_, window, cx| group.toggle_zoom(window, cx)
                }),
        )
        .child(options(panel, workbench, area))
        .into_any_element()
}

fn options(
    panel: Entity<ConversationPanel>,
    workbench: &WeakEntity<Workbench>,
    area: &WeakEntity<DockArea>,
) -> impl IntoElement {
    let workbench = workbench.clone();
    let area = area.clone();
    Button::new("conversation-options")
        .xsmall()
        .ghost()
        .icon(IconName::Ellipsis)
        .tooltip(crate::text::t("native.workbench.conversationOptions"))
        .dropdown_menu(move |menu, _, cx| {
            let id = panel.read(cx).session.id.to_string();
            let title = panel
                .read(cx)
                .session
                .conversation
                .as_ref()
                .map(|metadata| metadata.title.clone())
                .unwrap_or_default();
            let lifecycle = workbench.upgrade().map(|owner| owner.read(cx).lifecycle);
            let busy = workbench
                .upgrade()
                .is_none_or(|owner| owner.read(cx).is_closing(cx));
            let rename = workbench.clone();
            let refresh = panel.downgrade();
            let refresh_owner = workbench.clone();
            let settings = workbench.clone();
            let area = area.clone();
            let locked = area.upgrade().is_none_or(|area| area.read(cx).is_locked());
            let close_owner = workbench.clone();
            menu.item(
                PopupMenuItem::new(crate::text::t("native.workbench.renameConversation"))
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        let _ = rename.update(cx, |view, cx| {
                            if Some(view.lifecycle) == lifecycle {
                                view.rename_conversation(id.clone(), title.clone(), window, cx);
                            }
                        });
                    }),
            )
            .item(
                PopupMenuItem::new(crate::text::t("native.workbench.refreshConversation"))
                    .disabled(busy || panel.read(cx).is_refreshing())
                    .on_click(move |_, window, cx| {
                        if refresh_owner.upgrade().is_some_and(|owner| {
                            let view = owner.read(cx);
                            Some(view.lifecycle) == lifecycle && !view.is_closing(cx)
                        }) {
                            let _ = refresh.update(cx, |view, cx| view.refresh(window, cx));
                        }
                    }),
            )
            .item(
                PopupMenuItem::new(crate::text::t("native.workbench.modelSettings")).on_click(
                    move |_, window, cx| {
                        let _ = settings.update(cx, |view, cx| view.show_settings(window, cx));
                    },
                ),
            )
            .separator()
            .item(
                PopupMenuItem::new(crate::text::t("native.workbench.closeConversationColumn"))
                    .disabled(locked || busy)
                    .on_click(move |_, window, cx| {
                        if close_owner
                            .upgrade()
                            .is_some_and(|owner| Some(owner.read(cx).lifecycle) == lifecycle)
                        {
                            close(&close_owner, window, cx);
                        }
                    }),
            )
        })
}

pub(super) fn capture_close(
    frame: gpui::Stateful<gpui::Div>,
    workbench: &WeakEntity<Workbench>,
) -> gpui::Stateful<gpui::Div> {
    let workbench = workbench.clone();
    frame.capture_action(move |_: &ClosePanel, window, cx| {
        cx.stop_propagation();
        close(&workbench, window, cx);
    })
}
