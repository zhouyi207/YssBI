//! The editor region may close its last tab while DockArea retains lifecycle ownership.
use super::super::layout::columns;
use gpui::{AnyElement, App, Focusable, IntoElement, WeakEntity, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Selectable, Sizable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    dock::{
        AnyDrag, BasePanelView, ClosePanel, DockArea, DockLayout, DockPlacement, DragPanel,
        PanelHandle, PanelId, TabGroupContext, ToggleZoom,
    },
    menu::DropdownMenu,
    tab::{Tab, TabBar},
};
use gpui_kit_assets::IconName;
use std::sync::Arc;

pub(super) fn panel(
    group: &TabGroupContext,
    area: &WeakEntity<DockArea>,
    cx: &App,
) -> Option<Arc<dyn BasePanelView>> {
    let panel = group.active_panel()?;
    let area = area.upgrade()?;
    let area = area.read(cx);
    let tree = area.layout(DockPlacement::Center)?;
    (!area.is_locked()
        && !group.is_collapsed()
        && (tree.panels().eq([panel.panel_id(cx)])
            || (group.panels().len() == 1 && columns::last_editor(area, panel.panel_id(cx), cx))))
    .then(|| panel.clone())
}

pub(super) fn close(
    area: &WeakEntity<DockArea>,
    group: &TabGroupContext,
    id: PanelId,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(area) = area.upgrade() else {
        return;
    };
    let dock = area.read(cx);
    if dock.is_locked() || dock.panel(id).is_none_or(|panel| !panel.closable(cx)) {
        return;
    }
    if dock
        .layout(DockPlacement::Center)
        .is_some_and(|tree| tree.panels().eq([id]))
    {
        // The library rejects closing the last tab. Replacing the now-empty
        // center uses its public lifecycle path, including on_removed and layout persistence.
        area.update(cx, |dock, cx| {
            dock.set_center(DockLayout::tabs(), window, cx);
            dock.focus_handle(cx).focus(window, cx);
        });
    } else {
        area.update(cx, |dock, cx| {
            columns::prepare_editor_close(dock, id, window, cx)
        });
        group.close(id, window, cx);
    }
}

pub(super) fn render(
    panel: Arc<dyn BasePanelView>,
    group: &TabGroupContext,
    area: &WeakEntity<DockArea>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let handle = PanelHandle::of(&panel);
    let title = match handle {
        Some(handle) => match handle.tab_name(cx) {
            Some(name) => div().child(name).into_any_element(),
            None => handle.title(window, cx),
        },
        None => div().child(panel.panel_name(cx)).into_any_element(),
    };
    let closable = panel.closable(cx);
    let zoomed = group.is_zoomed();
    let zoom_control = handle
        .and_then(|handle| handle.zoom_control(cx))
        .filter(|_| panel.zoomable(cx));
    let toolbar_zoom = zoomed || zoom_control.is_some_and(|control| control.toolbar_visible());
    let menu_zoom = zoomed || zoom_control.is_some_and(|control| control.menu_visible());
    let presentation = handle.map(PanelHandle::panel);
    TabBar::new("tab-bar")
        .child(
            Tab::new()
                .selected(true)
                .child(title)
                .when(closable, |tab| {
                    tab.suffix(
                        Button::new("close-last-editor-tab")
                            .icon(IconName::X)
                            .xsmall()
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .foreground(cx.theme().secondary_foreground)
                                    .hover(*cx.theme().tokens.secondary_hover)
                                    .active(*cx.theme().tokens.secondary_active),
                            )
                            .ml(-px(8.))
                            .mr_2()
                            .tab_stop(false)
                            .tooltip(crate::text::t("common.close"))
                            .on_click({
                                let area = area.clone();
                                let group = group.clone();
                                let id = panel.panel_id(cx);
                                move |_, window, cx| {
                                    cx.stop_propagation();
                                    close(&area, &group, id, window, cx);
                                }
                            }),
                    )
                })
                .on_click({
                    let group = group.clone();
                    move |_, window, cx| group.select_tab(group.active_ix(), window, cx)
                })
                .when(group.is_droppable(), |tab| {
                    tab.drag_over::<DragPanel>(|tab, _, _, cx| {
                        tab.rounded_l_none()
                            .border_l_2()
                            .border_r_0()
                            .border_color(cx.theme().drag_border)
                    })
                    .on_drop({
                        let group = group.clone();
                        move |drag: &DragPanel, window, cx| {
                            group.drop_panel(drag.clone(), Some(0), true, window, cx);
                        }
                    })
                    .on_drop({
                        let group = group.clone();
                        move |item: &AnyDrag, window, cx| {
                            group.drop_item(item.clone(), None, window, cx);
                        }
                    })
                }),
        )
        .last_empty_space(
            div()
                .id("tab-bar-empty-space")
                .h_full()
                .flex_grow_1()
                .min_w_16()
                .when(group.is_droppable(), |space| {
                    space
                        .drag_over::<DragPanel>(|space, _, _, cx| {
                            space.bg(cx.theme().tokens.drop_target)
                        })
                        .on_drop({
                            let group = group.clone();
                            move |drag: &DragPanel, window, cx| {
                                group.drop_panel(drag.clone(), None, false, window, cx);
                            }
                        })
                        .on_drop({
                            let group = group.clone();
                            move |item: &AnyDrag, window, cx| {
                                group.drop_item(item.clone(), None, window, cx);
                            }
                        })
                }),
        )
        .suffix(
            div()
                .flex()
                .items_center()
                .h_full()
                .px_2()
                .gap_1()
                .border_l_1()
                .border_b_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().tab_bar)
                .children(handle.and_then(|handle| handle.title_suffix(window, cx)))
                .children(
                    handle
                        .and_then(|handle| handle.toolbar_buttons(window, cx))
                        .into_iter()
                        .flatten()
                        .map(|button| button.xsmall().ghost().tab_stop(false)),
                )
                .when(toolbar_zoom, |toolbar| {
                    toolbar.child(
                        Button::new("zoom-panel")
                            .icon(if zoomed {
                                IconName::Minimize
                            } else {
                                IconName::Maximize
                            })
                            .xsmall()
                            .ghost()
                            .tab_stop(false)
                            .tooltip(if zoomed {
                                crate::text::t("native.workbench.restoreLayout")
                            } else {
                                crate::text::t("native.workbench.zoomPanel")
                            })
                            .on_click({
                                let group = group.clone();
                                move |_, window, cx| group.toggle_zoom(window, cx)
                            }),
                    )
                })
                .child(
                    Button::new("menu")
                        .icon(IconName::Ellipsis)
                        .xsmall()
                        .ghost()
                        .tab_stop(false)
                        .dropdown_menu(move |menu, window, cx| {
                            menu.when_some(presentation.clone(), |menu, panel| {
                                panel.dropdown_menu(menu, window, cx)
                            })
                            .separator()
                            .menu_with_disabled(
                                if zoomed {
                                    crate::text::t("native.workbench.restoreLayout")
                                } else {
                                    crate::text::t("native.workbench.zoomPanel")
                                },
                                Box::new(ToggleZoom),
                                !menu_zoom,
                            )
                            .when(closable, |menu| {
                                menu.separator()
                                    .menu(crate::text::t("common.close"), Box::new(ClosePanel))
                            })
                        })
                        .anchor(gpui::Anchor::TopRight),
                ),
        )
        .into_any_element()
}
