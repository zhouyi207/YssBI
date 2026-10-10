//! Centered header navigation projects the root DockArea's current panels.
use super::{Workbench, activity::ActivityPanel, layout::columns, menus::WorkbenchPanel};
use crate::plugins::PluginsSidebar;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    dock::{BasePanelView, DockArea, DockPlacement, NodeId, PaneRef, PanelHandle},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, App, Context, Empty, IntoElement, WeakEntity, Window, div, prelude::*, px,
};
use std::{any::TypeId, sync::Arc};

impl Workbench {
    fn select_sidebar_panel(
        &mut self,
        panel: WorkbenchPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        if panel != WorkbenchPanel::Assistant
            && let Some(intent) = self.assistant_intent.take()
        {
            self.finish_intent(&intent, false, window, cx);
        }
        if !columns::conversation_zoomed(self.dock.read(cx)) {
            self.dock
                .update(cx, |dock, cx| dock.set_zoomed_out(window, cx));
        }
        if !self.panel_is_displayed(panel, cx) {
            self.show_panel(panel, window, cx);
        }
    }
}

pub(super) fn is_navigation(panel: &Arc<dyn BasePanelView>) -> bool {
    let kind = panel.view().entity_type();
    kind == TypeId::of::<ActivityPanel>() || kind == TypeId::of::<PluginsSidebar>()
}

pub(super) fn render_header(
    node: NodeId,
    area: &WeakEntity<DockArea>,
    workbench: &WeakEntity<Workbench>,
    cx: &mut App,
) -> AnyElement {
    let Some(dock) = area.upgrade() else {
        return Empty.into_any_element();
    };
    let dock = dock.read(cx);
    let group = [
        DockPlacement::Left,
        DockPlacement::Center,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ]
    .into_iter()
    .filter_map(|placement| dock.layout(placement))
    .find_map(|tree| tree.find_node(node));
    let Some(PaneRef::Tabs {
        panels: ids,
        active_ix,
    }) = group.map(|group| group.kind())
    else {
        return Empty.into_any_element();
    };
    let active = ids.get(active_ix).copied();
    let panels = ids
        .iter()
        .filter_map(|id| dock.panel(*id).cloned())
        .collect::<Vec<_>>();
    let zoomed = dock.zoomed_group() == Some(node);
    let navigation_active = active
        .and_then(|id| dock.panel(id))
        .is_some_and(is_navigation);
    let other_panels = navigation_active
        && panels
            .iter()
            .any(|panel| panel.visible(cx) && !is_navigation(panel));
    let navigation = [
        (
            "sidebar-project",
            IconName::Folder,
            crate::text::t("activityBar.project"),
            WorkbenchPanel::Project,
        ),
        (
            "sidebar-nodes",
            IconName::Frame,
            crate::text::t("native.workbench.nodeCatalog"),
            WorkbenchPanel::Nodes,
        ),
        (
            "sidebar-assistant",
            IconName::MessageSquareText,
            crate::text::t("panel.assistant"),
            WorkbenchPanel::Assistant,
        ),
        (
            "sidebar-plugins",
            IconName::Puzzle,
            crate::text::t("activityBar.plugins"),
            WorkbenchPanel::Plugins,
        ),
    ]
    .map(|(id, icon, title, panel)| {
        let (active, disabled) = workbench.upgrade().map_or((false, true), |owner| {
            let view = owner.read(cx);
            let target = view.dock_panel(panel);
            let active = target
                .as_ref()
                .is_some_and(|(target, _)| active == Some(target.panel_id(cx)));
            let unavailable = panel != WorkbenchPanel::Assistant && target.is_none();
            (active, unavailable || view.is_closing(cx))
        });
        div()
            .relative()
            .h_full()
            .flex()
            .items_center()
            .child(
                Button::new(id)
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .foreground(if active {
                                cx.theme().sidebar_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .hover(cx.theme().sidebar_accent.opacity(0.5))
                            .active(cx.theme().sidebar_accent.opacity(0.5)),
                    )
                    .size(px(28.))
                    .p_0()
                    .rounded(px(4.))
                    // A child icon keeps its explicit pixel size independent of Button's size.
                    .child(Icon::new(icon).with_size(px(16.)))
                    .accessibility_label(title)
                    .tooltip(title)
                    .selected(active)
                    .disabled(disabled)
                    .on_click({
                        let workbench = workbench.clone();
                        move |_, window, cx| {
                            let _ = workbench.update(cx, |view, cx| {
                                view.select_sidebar_panel(panel, window, cx);
                            });
                        }
                    }),
            )
            .when(active, |item| {
                item.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left(px(6.))
                        .right(px(6.))
                        .h(px(2.))
                        .bg(cx.theme().sidebar_primary),
                )
            })
    });
    let controls = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .justify_end()
        .gap_1()
        // A resource docked beside navigation must stay reachable after its tabs are hidden.
        .when(other_panels, |header| {
            header.child(
                Button::new("sidebar-panel-picker")
                    .xsmall()
                    .ghost()
                    .icon(IconName::ChevronDown)
                    .tooltip(crate::text::t("native.workbench.switchPanel"))
                    .dropdown_menu({
                        let area = area.clone();
                        let panels = panels.clone();
                        move |mut menu, _, cx| {
                            for panel in &panels {
                                if !panel.visible(cx) {
                                    continue;
                                }
                                let panel = panel.clone();
                                let area = area.clone();
                                let id = panel.panel_id(cx);
                                menu = menu.item(
                                    PopupMenuItem::element(move |window, cx| {
                                        div().children(
                                            PanelHandle::of(&panel)
                                                .map(|panel| panel.title(window, cx)),
                                        )
                                    })
                                    .on_click(
                                        move |_, window, cx| {
                                            let _ = area.update(cx, |dock, cx| {
                                                dock.select_panel(id, window, cx)
                                            });
                                        },
                                    ),
                                );
                            }
                            menu
                        }
                    }),
            )
        })
        .when(navigation_active && zoomed, |header| {
            header.child(
                Button::new("sidebar-restore-layout")
                    .xsmall()
                    .ghost()
                    .icon(IconName::Minimize)
                    .tooltip(crate::text::t("native.workbench.restoreLayout"))
                    .on_click({
                        let area = area.clone();
                        move |_, window, cx| {
                            let _ = area.update(cx, |dock, cx| dock.set_zoomed_out(window, cx));
                        }
                    }),
            )
        });
    div()
        .id("sidebar-header")
        .h(px(36.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_1()
        .border_b_1()
        .border_color(cx.theme().sidebar_border)
        .bg(cx.theme().sidebar)
        .text_color(cx.theme().sidebar_foreground)
        .child(div().flex_1().min_w_0())
        .child(
            div()
                .flex()
                .h_full()
                .flex_shrink_0()
                .items_center()
                .gap(px(4.))
                .children(navigation),
        )
        .child(controls)
        .into_any_element()
}
