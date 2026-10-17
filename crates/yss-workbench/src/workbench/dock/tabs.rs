use super::super::layout::columns;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Selectable, Sizable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    dock::{
        AnyDrag, BasePanelView, ClosePanel, DockArea, DockLayout, DockPlacement, DragPanel,
        PanelHandle, PanelId, TabGroupContext, ToggleZoom,
    },
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    tab::{Tab, TabBar},
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Div, Empty, Focusable, IntoElement, Render, ScrollHandle,
    Stateful, WeakEntity, Window, actions, div, prelude::*, px, size,
};
use std::{cell::Cell, sync::Arc};

actions!(native_workbench, [CloseTab, CloseOtherTabs, CloseAllTabs]);

#[derive(Default)]
pub(super) struct Tabs {
    scroll_handle: ScrollHandle,
    displayed_panel: Cell<Option<(PanelId, usize)>>,
}

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

#[derive(Clone, Copy)]
enum CloseTabs {
    Current,
    Others,
    Left,
    Right,
    All,
}

fn scope_targets(
    group: &TabGroupContext,
    target: PanelId,
    scope: CloseTabs,
    cx: &App,
) -> Option<Vec<PanelId>> {
    let target_index = group
        .panels()
        .iter()
        .position(|panel| panel.panel_id(cx) == target)?;
    Some(
        group
            .panels()
            .iter()
            .enumerate()
            .filter_map(|(index, panel)| {
                let included = match scope {
                    CloseTabs::Current => index == target_index,
                    CloseTabs::Others => index != target_index,
                    CloseTabs::Left => index < target_index,
                    CloseTabs::Right => index > target_index,
                    CloseTabs::All => true,
                };
                (included && panel.visible(cx)).then(|| panel.panel_id(cx))
            })
            .collect(),
    )
}

fn close_scope(
    area: &WeakEntity<DockArea>,
    group: &TabGroupContext,
    target: PanelId,
    scope: CloseTabs,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(targets) = scope_targets(group, target, scope, cx)
        && targets_closable(area, group, &targets, cx)
    {
        close_next(area.clone(), group.clone(), targets.into_iter(), window, cx);
    }
}

/// Application-menu and keyboard tab commands act on the group's active editor.
pub(super) fn handle_close_actions(
    frame: Stateful<Div>,
    area: &WeakEntity<DockArea>,
    group: &TabGroupContext,
    cx: &App,
) -> Stateful<Div> {
    let Some(target) = group.active_panel().map(|panel| panel.panel_id(cx)) else {
        return frame;
    };
    let editor_group = area.upgrade().is_some_and(|area| {
        area.read(cx)
            .layout(DockPlacement::Center)
            .is_some_and(|tree| tree.find_node(group.node()).is_some())
    });
    if !editor_group {
        return frame;
    }
    let handler = |scope| {
        let area = area.clone();
        let group = group.clone();
        move |window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            close_scope(&area, &group, target, scope, window, cx);
        }
    };
    let current = handler(CloseTabs::Current);
    let others = handler(CloseTabs::Others);
    let all = handler(CloseTabs::All);
    frame
        .on_action(move |_: &CloseTab, window, cx| current(window, cx))
        .on_action(move |_: &CloseOtherTabs, window, cx| others(window, cx))
        .on_action(move |_: &CloseAllTabs, window, cx| all(window, cx))
}

fn tab_menu(
    mut menu: PopupMenu,
    area: &WeakEntity<DockArea>,
    group: &TabGroupContext,
    target: PanelId,
    cx: &App,
) -> PopupMenu {
    for (scope, label) in [
        (CloseTabs::Current, "common.close"),
        (CloseTabs::Others, "native.workbench.closeOtherTabs"),
        (CloseTabs::Left, "native.workbench.closeTabsToLeft"),
        (CloseTabs::Right, "native.workbench.closeTabsToRight"),
        (CloseTabs::All, "native.workbench.closeAllTabs"),
    ] {
        let Some(targets) = scope_targets(group, target, scope, cx) else {
            return menu;
        };
        let disabled = targets.is_empty() || !targets_closable(area, group, &targets, cx);
        let area = area.clone();
        let group = group.clone();
        menu = menu.item(
            PopupMenuItem::new(crate::text::t(label))
                .disabled(disabled)
                .on_click(move |_, window, cx| {
                    close_scope(&area, &group, target, scope, window, cx);
                }),
        );
    }
    if group
        .panels()
        .iter()
        .any(|panel| panel.visible(cx) && !panel.closable(cx))
    {
        menu = menu
            .separator()
            .label(crate::text::t("native.workbench.tabsCloseProtected"));
    }
    menu
}

fn targets_closable(
    area: &WeakEntity<DockArea>,
    group: &TabGroupContext,
    targets: &[PanelId],
    cx: &App,
) -> bool {
    let Some(area_entity) = area.upgrade() else {
        return false;
    };
    let dock = area_entity.read(cx);
    let last = panel(group, area, cx).map(|panel| panel.panel_id(cx));
    !dock.is_locked()
        && !group.is_locked()
        && targets.iter().all(|id| {
            dock.layout(DockPlacement::Center)
                .is_some_and(|tree| tree.find_panel_node(*id) == Some(group.node()))
                && dock.panel(*id).is_some_and(|panel| panel.closable(cx))
                && (group.is_panel_closable(*id, cx) || last == Some(*id))
        })
}

fn close_next(
    area: WeakEntity<DockArea>,
    group: TabGroupContext,
    mut targets: std::vec::IntoIter<PanelId>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(id) = targets.next() else { return };
    if !targets_closable(&area, &group, &[id], cx) {
        return;
    }
    close(&area, &group, id, window, cx);
    if targets.len() > 0 {
        // Dock applies ClosePanel events after this callback. Let that lifecycle
        // finish before deciding whether the next target is the last editor.
        window.defer(cx, move |window, cx| {
            close_next(area, group, targets, window, cx)
        });
    }
}

impl Tabs {
    pub(super) fn render(
        &self,
        group: &TabGroupContext,
        area: &WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let Some(active_panel) = group.active_panel() else {
            return Empty.into_any_element();
        };
        let active_id = active_panel.panel_id(cx);
        let displayed = (active_id, group.active_ix());
        if self.displayed_panel.replace(Some(displayed)) != Some(displayed)
            && let Some(index) = group
                .panels()
                .iter()
                .filter(|panel| panel.visible(cx))
                .position(|panel| panel.panel_id(cx) == active_id)
        {
            self.scroll_handle.scroll_to_item(index);
        }
        let last_editor = panel(group, area, cx).map(|panel| panel.panel_id(cx));
        let handle = PanelHandle::of(active_panel);
        let closable = group.is_panel_closable(active_id, cx)
            || (last_editor == Some(active_id) && active_panel.closable(cx));
        let zoomed = group.is_zoomed();
        let zoom_control = handle
            .and_then(|handle| handle.zoom_control(cx))
            .filter(|_| active_panel.zoomable(cx));
        let toolbar_zoom = zoomed || zoom_control.is_some_and(|control| control.toolbar_visible());
        let menu_zoom = zoomed || zoom_control.is_some_and(|control| control.menu_visible());
        let presentation = handle.map(PanelHandle::panel);
        let editor_group = area.upgrade().is_some_and(|area| {
            area.read(cx)
                .layout(DockPlacement::Center)
                .is_some_and(|tree| tree.find_node(group.node()).is_some())
        });
        let menu_area = area.clone();
        let menu_group = group.clone();
        TabBar::new("tab-bar")
            .small()
            .h(super::HEADER_HEIGHT)
            .flex_shrink_0()
            .track_scroll(&self.scroll_handle)
            .children(
                group
                    .panels()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, panel)| {
                        if !panel.visible(cx) {
                            return None;
                        }
                        let id = panel.panel_id(cx);
                        let handle = PanelHandle::of(panel);
                        let tab_name = handle.and_then(|handle| handle.tab_name(cx));
                        let title = match handle {
                            Some(handle) => match tab_name.clone() {
                                Some(name) => div().child(name).into_any_element(),
                                None => handle.title(window, cx),
                            },
                            None => div().child(panel.panel_name(cx)).into_any_element(),
                        };
                        let closable = group.is_panel_closable(id, cx)
                            || (last_editor == Some(id) && panel.closable(cx));
                        let drag = group
                            .is_draggable()
                            .then(|| group.drag_panel(index, cx))
                            .flatten();
                        Some(
                            Tab::new()
                                .aria_label(tab_name.unwrap_or_else(|| panel.panel_name(cx).into()))
                                // Tab applies its size-tier height during render.
                                .min_h(super::HEADER_HEIGHT)
                                .max_h(super::HEADER_HEIGHT)
                                .selected(id == active_id)
                                .child(title)
                                .relative()
                                .when(editor_group, |tab| {
                                    // TabBar accepts Tab rather than a ContextMenu wrapper.
                                    // A full-tab child keeps the component's pointer/drag behavior.
                                    tab.child(
                                        div()
                                            .id(("editor-tab-context", id.as_u64()))
                                            .absolute()
                                            .inset_0()
                                            .context_menu({
                                                let area = area.clone();
                                                let group = group.clone();
                                                move |menu, _, cx| {
                                                    tab_menu(menu, &area, &group, id, cx)
                                                }
                                            }),
                                    )
                                })
                                .when(closable, |tab| {
                                    tab.suffix(
                                        Button::new(("close-tab", id.as_u64()))
                                            .icon(IconName::X)
                                            .xsmall()
                                            .custom(
                                                ButtonCustomVariant::new(cx)
                                                    .foreground(cx.theme().secondary_foreground)
                                                    .hover(*cx.theme().tokens.secondary_hover)
                                                    .active(*cx.theme().tokens.secondary_active),
                                            )
                                            .ml(-px(6.))
                                            .mr_2()
                                            .tab_stop(false)
                                            .tooltip(crate::text::t("common.close"))
                                            .accessibility_label(crate::text::t("common.close"))
                                            .on_click({
                                                let area = area.clone();
                                                let group = group.clone();
                                                move |_, window, cx| {
                                                    cx.stop_propagation();
                                                    if last_editor == Some(id) {
                                                        close(&area, &group, id, window, cx);
                                                    } else {
                                                        group.close(id, window, cx);
                                                    }
                                                }
                                            }),
                                    )
                                })
                                .on_click({
                                    let group = group.clone();
                                    move |_, window, cx| group.select_tab(index, window, cx)
                                })
                                .when_some(drag, |tab, drag| {
                                    tab.on_drag(drag, {
                                        let panel = panel.clone();
                                        move |drag, offset, window, cx| {
                                            cx.stop_propagation();
                                            drag.set_drag_offset(offset);
                                            drag.set_preview_size(size(
                                                px(96.),
                                                super::HEADER_HEIGHT.to_pixels(window.rem_size()),
                                            ));
                                            cx.new(|_| TabPreview {
                                                panel: panel.clone(),
                                            })
                                        }
                                    })
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
                                            group.drop_panel(
                                                drag.clone(),
                                                Some(index),
                                                true,
                                                window,
                                                cx,
                                            );
                                        }
                                    })
                                    .drag_over::<AnyDrag>(|tab, _, _, cx| {
                                        tab.rounded_l_none()
                                            .border_l_2()
                                            .border_r_0()
                                            .border_color(cx.theme().drag_border)
                                    })
                                    .on_drop({
                                        let group = group.clone();
                                        move |item: &AnyDrag, window, cx| {
                                            group.drop_item(item.clone(), None, window, cx);
                                        }
                                    })
                                }),
                        )
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
                            .drag_over::<AnyDrag>(|space, _, _, cx| {
                                space.bg(cx.theme().tokens.drop_target)
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
                                .accessibility_label(if zoomed {
                                    crate::text::t("native.workbench.restoreLayout")
                                } else {
                                    crate::text::t("native.workbench.zoomPanel")
                                })
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
                            .tab_stop(editor_group)
                            .accessibility_label(crate::text::t("native.canvas.moreActions"))
                            .dropdown_menu(move |menu, window, cx| {
                                menu.when_some(presentation.clone(), |menu, panel| {
                                    panel.dropdown_menu(menu, window, cx)
                                })
                                .when(editor_group, |menu| {
                                    tab_menu(
                                        menu.separator(),
                                        &menu_area,
                                        &menu_group,
                                        active_id,
                                        cx,
                                    )
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
                                .when(
                                    closable && !editor_group,
                                    |menu| {
                                        menu.separator().menu(
                                            crate::text::t("common.close"),
                                            Box::new(ClosePanel),
                                        )
                                    },
                                )
                            })
                            .anchor(gpui_kit::Anchor::TopRight),
                    ),
            )
            .into_any_element()
    }
}

struct TabPreview {
    panel: Arc<dyn BasePanelView>,
}

impl Render for TabPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match PanelHandle::of(&self.panel) {
            Some(handle) => handle.title(window, cx),
            None => div().child(self.panel.panel_name(cx)).into_any_element(),
        };
        div()
            .w_24()
            .h(super::HEADER_HEIGHT)
            .px_3()
            .flex()
            .items_center()
            .overflow_hidden()
            .whitespace_nowrap()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .text_color(cx.theme().tab_foreground)
            .bg(cx.theme().tab_active)
            .opacity(0.75)
            .child(title)
    }
}
