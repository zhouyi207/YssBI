//! Centered header navigation projects the root DockArea's current panels.
use super::{Workbench, activity::ActivityPanel, layout::columns, menus::WorkbenchPanel};
use crate::plugins::PluginsSidebar;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, Sizable, Theme,
    button::{Button, ButtonVariants},
    dock::{BasePanelView, DockArea, DockPlacement, NodeId, PaneRef, PanelHandle},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Empty, Entity, EntityId, IntoElement, Rems, Render,
    StyleRefinement, Subscription, WeakEntity, Window, div, prelude::*, rems,
};
use std::{any::TypeId, cell::RefCell, collections::BTreeMap, sync::Arc};

const NAVIGATION_BUTTON_WIDTH: Rems = rems(2.);
const NAVIGATION_GAP: Rems = rems(0.25);

/// Shared by the regular and conversation-zoom dock renderers. Each group keeps
/// its control identity until that group leaves the dock topology.
#[derive(Default)]
pub(super) struct NavigationHeaders {
    groups: RefCell<BTreeMap<NodeId, Entity<Navigation>>>,
}

impl NavigationHeaders {
    pub(super) fn retain_groups(&self, dock: &DockArea) {
        self.groups.borrow_mut().retain(|node, _| {
            placements()
                .into_iter()
                .filter_map(|placement| dock.layout(placement))
                .any(|tree| tree.find_node(*node).is_some())
        });
    }

    fn render(
        &self,
        node: NodeId,
        active: Option<EntityId>,
        workbench: &WeakEntity<Workbench>,
        cx: &mut App,
    ) -> AnyElement {
        // Read broad owners outside the cached view. A graph notification can
        // redraw the dock without changing any of these four controls.
        let state = NavigationState::read(active, workbench, cx);
        let mut groups = self.groups.borrow_mut();
        let navigation = groups.entry(node).or_insert_with(|| {
            cx.new(|cx| Navigation {
                state,
                workbench: workbench.clone(),
                _theme: cx.observe_global::<Theme>(|_, cx| cx.notify()),
            })
        });
        if navigation.read(cx).state != state {
            navigation.update(cx, |navigation, cx| {
                navigation.state = state;
                cx.notify();
            });
        }
        // Entity alone is not a rendering boundary. GPUI requires an explicit
        // cached view with definite bounds; the parent supplies the row height.
        navigation
            .clone()
            .cached(
                StyleRefinement::default()
                    .w(NAVIGATION_BUTTON_WIDTH * 4. + NAVIGATION_GAP * 3.)
                    .h_full()
                    .flex_shrink_0(),
            )
            .into_any_element()
    }
}

fn placements() -> [DockPlacement; 4] {
    [
        DockPlacement::Left,
        DockPlacement::Center,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ]
}

#[derive(Clone, Copy, PartialEq)]
struct NavigationState {
    buttons: [(bool, bool); 4],
    locale: &'static str,
}

impl NavigationState {
    fn read(active: Option<EntityId>, workbench: &WeakEntity<Workbench>, cx: &App) -> Self {
        let buttons = workbench.upgrade().map_or([(false, true); 4], |owner| {
            let view = owner.read(cx);
            let closing = view.is_closing(cx);
            let targets = ["project", "nodes", "assistant"]
                .map(|key| view.activities.get(key).and_then(WeakEntity::upgrade));
            let targets = [
                targets[0].as_ref().map(Entity::entity_id),
                targets[1].as_ref().map(Entity::entity_id),
                targets[2].as_ref().map(Entity::entity_id),
                Some(view.plugins_sidebar.entity_id()),
            ];
            std::array::from_fn(|index| {
                let target = targets[index];
                (
                    target.is_some() && target == active,
                    closing || (index != 2 && target.is_none()),
                )
            })
        });
        Self {
            buttons,
            locale: crate::text::locale(),
        }
    }
}

struct Navigation {
    state: NavigationState,
    workbench: WeakEntity<Workbench>,
    _theme: Subscription,
}

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
    headers: &NavigationHeaders,
    cx: &mut App,
) -> AnyElement {
    let Some(dock) = area.upgrade() else {
        return Empty.into_any_element();
    };
    let dock = dock.read(cx);
    let group = placements()
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
    let zoomed = dock.zoomed_group() == Some(node);
    let navigation_active = active
        .and_then(|id| dock.panel(id))
        .is_some_and(is_navigation);
    let other_panels = navigation_active
        && ids
            .iter()
            .filter_map(|id| dock.panel(*id))
            .any(|panel| panel.visible(cx) && !is_navigation(panel));
    let active_entity = active
        .and_then(|id| dock.panel(id))
        .map(|panel| panel.view().entity_id());
    let navigation = headers.render(node, active_entity, workbench, cx);
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
                        move |mut menu, _, cx| {
                            let Some(dock) = area.upgrade() else {
                                return menu;
                            };
                            let dock = dock.read(cx);
                            let group = placements()
                                .into_iter()
                                .filter_map(|placement| dock.layout(placement))
                                .find_map(|tree| tree.find_node(node));
                            let Some(PaneRef::Tabs { panels, .. }) =
                                group.map(|group| group.kind())
                            else {
                                return menu;
                            };
                            for panel in panels.iter().filter_map(|id| dock.panel(*id)) {
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
        .h(super::dock::HEADER_HEIGHT)
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_1()
        .border_b_1()
        .border_color(cx.theme().sidebar_border)
        .bg(cx.theme().sidebar)
        .text_color(cx.theme().sidebar_foreground)
        .child(div().flex_1().min_w_0())
        .child(navigation)
        .child(controls)
        .into_any_element()
}

impl Render for Navigation {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_navigation(self)
    }
}

fn render_navigation(view: &Navigation) -> AnyElement {
    let navigation = [
        (
            "sidebar-project",
            IconName::Folder,
            crate::text::t("activityBar.project"),
            WorkbenchPanel::Project,
        ),
        (
            "sidebar-nodes",
            IconName::Workflow,
            crate::text::t("native.workbench.nodeCatalog"),
            WorkbenchPanel::Nodes,
        ),
        (
            "sidebar-assistant",
            IconName::Bot,
            crate::text::t("panel.assistant"),
            WorkbenchPanel::Assistant,
        ),
        (
            "sidebar-plugins",
            IconName::Blocks,
            crate::text::t("activityBar.plugins"),
            WorkbenchPanel::Plugins,
        ),
    ]
    .into_iter()
    .zip(view.state.buttons)
    .map(|((id, icon, title, panel), (active, disabled))| {
        Button::new(id)
            .ghost()
            .with_size(super::dock::HEADER_BUTTON_SIZE)
            .w(NAVIGATION_BUTTON_WIDTH)
            .h_full()
            .icon(icon)
            .accessibility_label(title.clone())
            .tooltip(title)
            .selected(active)
            .toggled(active)
            .disabled(disabled)
            .on_click({
                let workbench = view.workbench.clone();
                move |_, window, cx| {
                    let Some(workbench) = workbench.upgrade() else {
                        return;
                    };
                    workbench.update(cx, |view, cx| {
                        view.select_sidebar_panel(panel, window, cx);
                    });
                }
            })
    });
    div()
        .flex()
        .size_full()
        .items_center()
        .gap(NAVIGATION_GAP)
        .py_0p5()
        .children(navigation)
        .into_any_element()
}
