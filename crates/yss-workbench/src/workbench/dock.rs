//! Workbench dock appearance; all topology and interaction state stays in DockArea.
mod conversation;
mod regions;
mod tabs;
mod watermark;
mod zoom_sidebar;
pub(super) use watermark::EmptyEditor;

use super::{
    LogsPanel, OutputPanel, ProblemsPanel, ResultsPanel, Workbench, sidebar,
    status_bar::{ColumnStatusBar, STATUS_BAR_HEIGHT, StatusColumn},
};
use gpui_kit::base::ResizeHandleContext;
use gpui_kit::component::ElementExt;
use gpui_kit::component::dock::{
    BasePanelView, ClosePanel, DockArea, DockAreaRenderer, DockContext, DockPlacement, DockSkin,
    DropIndicator, NodeId, PanelState, PanelStyle, TabGroupContext, TabGroupRenderer,
};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext, Axis, Div, Empty, Entity, IntoElement, Pixels, Stateful,
    Styled, WeakEntity, Window, div, prelude::*, px,
};
use std::{any::TypeId, cell::Cell, rc::Rc, sync::Arc};

pub(super) use yss_ui::appearance::HEADER_HEIGHT;
pub(super) const HEADER_BUTTON_SIZE: Pixels = px(20.);

pub(super) fn create(
    workbench: WeakEntity<Workbench>,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<DockArea>, Rc<WorkbenchDock>) {
    let mut renderer = None;
    let area = cx.new(|cx| {
        let skin = Rc::new(WorkbenchDock {
            skin: DockSkin::new(cx),
            area: cx.weak_entity(),
            workbench,
            center_bottom_margin: Cell::new(px(0.)),
            measurements: Rc::new(regions::Measurements::default()),
            navigation_headers: Rc::new(sidebar::NavigationHeaders::default()),
        });
        renderer = Some(skin.clone());
        DockArea::new("yssbi-workbench", None, window, cx).with_renderer(skin)
    });
    let renderer = renderer.expect("WorkbenchDock was created with its DockArea");
    renderer.skin.set_panel_style(PanelStyle::TabBar, cx);
    renderer.skin.set_close_button_visible(true, cx);
    renderer.skin.set_toggle_button_visible(false, cx);
    (area, renderer)
}

pub(super) struct WorkbenchDock {
    skin: Rc<DockSkin>,
    area: WeakEntity<DockArea>,
    workbench: WeakEntity<Workbench>,
    center_bottom_margin: Cell<Pixels>,
    measurements: Rc<regions::Measurements>,
    navigation_headers: Rc<sidebar::NavigationHeaders>,
}

impl WorkbenchDock {
    pub(super) fn update_style(&self, area: &DockArea, cx: &App) {
        self.navigation_headers.retain_groups(area);
        let hide_strip = !area.is_zoomed()
            && !area.is_dock_open(DockPlacement::Bottom)
            && area.layout(DockPlacement::Bottom).is_some_and(|tree| {
                tree.panels().all(|id| {
                    area.panel(id)
                        .is_none_or(|panel| !panel.visible(cx) || is_tool_panel(panel))
                })
            });
        // gpui-base 0.7.1 reserves a non-configurable 29px strip for collapsed bottom tabs.
        // Extend only the center column into that strip; DockArea still owns all sizes.
        self.center_bottom_margin
            .set(if hide_strip { -px(29.) } else { px(0.) });
    }
}

fn is_tool_panel(panel: &Arc<dyn BasePanelView>) -> bool {
    let kind = panel.view().entity_type();
    kind == TypeId::of::<ProblemsPanel>()
        || kind == TypeId::of::<OutputPanel>()
        || kind == TypeId::of::<ResultsPanel>()
        || kind == TypeId::of::<LogsPanel>()
}

impl DockAreaRenderer for WorkbenchDock {
    fn frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.skin.frame(window, cx).child(zoom_sidebar::Sidebar {
            area: self.area.clone(),
            workbench: self.workbench.clone(),
            navigation_headers: self.navigation_headers.clone(),
        })
    }

    fn center_frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let area = self.area.clone();
        let owner = self.workbench.clone();
        let measurements = self.measurements.clone();
        self.skin
            .center_frame(window, cx)
            .mb(self.center_bottom_margin.get())
            .relative()
            .pb(STATUS_BAR_HEIGHT)
            .child(regions::status_bars(
                self.area.clone(),
                self.workbench.clone(),
                self.measurements.clone(),
                -self.center_bottom_margin.get(),
            ))
            .on_prepaint(move |bounds, window, cx| {
                let has_conversation = area.upgrade().is_some_and(|area| {
                    super::layout::columns::conversation_edge(area.read(cx)).is_some()
                });
                measurements.record_center(bounds, has_conversation, &owner, window, cx);
            })
    }

    fn split_frame(
        &self,
        node: NodeId,
        axis: Axis,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        let area = self.area.clone();
        let owner = self.workbench.clone();
        let measurements = self.measurements.clone();
        self.skin
            .split_frame(node, axis, window, cx)
            .relative()
            .on_prepaint(move |bounds, window, cx| {
                super::layout::columns::sizing::fit(&area, node, bounds.size.width, window, cx);
                if area.upgrade().is_some_and(|area| {
                    super::layout::columns::conversation_edge(area.read(cx)) == Some(node)
                }) {
                    measurements.record_edge(bounds.right(), &owner, window, cx);
                }
            })
            .child(watermark::Watermark {
                area: self.area.clone(),
                node,
            })
    }

    fn render_split_handle(
        &self,
        handle: &ResizeHandleContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.skin.render_split_handle(handle, window, cx)
    }

    fn render_dock(
        &self,
        dock: &DockContext,
        content: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let content = self.skin.render_dock(dock, content, window, cx);
        if dock.placement() == DockPlacement::Bottom {
            regions::RegionContent::bottom(
                content,
                self.area.clone(),
                self.workbench.clone(),
                self.measurements.clone(),
            )
            .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().flex_1().min_h_0().min_w_0().child(content))
                .child(ColumnStatusBar::new(
                    self.workbench.clone(),
                    if dock.placement() == DockPlacement::Left {
                        StatusColumn::Left
                    } else {
                        StatusColumn::Right
                    },
                ))
                .into_any_element()
        }
    }

    fn build_placeholder(
        &self,
        state: &PanelState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<dyn BasePanelView>> {
        self.skin.build_placeholder(state, window, cx)
    }

    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(WorkbenchTabs {
            skin: self.skin.tab_group_renderer(),
            tabs: tabs::Tabs::default(),
            area: self.area.clone(),
            workbench: self.workbench.clone(),
            measurements: self.measurements.clone(),
            navigation_headers: self.navigation_headers.clone(),
        })
    }
}

struct WorkbenchTabs {
    skin: Rc<dyn TabGroupRenderer>,
    tabs: tabs::Tabs,
    area: WeakEntity<DockArea>,
    workbench: WeakEntity<Workbench>,
    measurements: Rc<regions::Measurements>,
    navigation_headers: Rc<sidebar::NavigationHeaders>,
}

impl TabGroupRenderer for WorkbenchTabs {
    fn frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let area = self.area.clone();
        let owner = self.workbench.clone();
        let measurements = self.measurements.clone();
        let node = group.node();
        let frame = self
            .skin
            .frame(group, window, cx)
            .on_prepaint(move |bounds, window, cx| {
                if area.upgrade().is_some_and(|area| {
                    super::layout::columns::conversation_edge(area.read(cx)) == Some(node)
                }) {
                    measurements.record_edge(bounds.right(), &owner, window, cx);
                }
            });
        // The zoomed group is the sole center column, not an ordinary tab footer.
        let frame = frame.when(group.is_zoomed(), |frame| {
            frame.relative().pb(STATUS_BAR_HEIGHT).child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .child(ColumnStatusBar::new(
                        self.workbench.clone(),
                        if conversation::matches(group) {
                            StatusColumn::Conversation
                        } else {
                            StatusColumn::Editor
                        },
                    )),
            )
        });
        if conversation::matches(group) {
            return conversation::capture_close(frame, &self.workbench);
        }
        frame.when_some(tabs::panel(group, &self.area, cx), |frame, panel| {
            let area = self.area.clone();
            let group = group.clone();
            let id = panel.panel_id(cx);
            // DockSkin's bubble handler consumes ClosePanel even when it refuses
            // the last tab, so handle this case before that handler runs.
            frame.capture_action(move |_: &ClosePanel, window, cx| {
                cx.stop_propagation();
                tabs::close(&area, &group, id, window, cx);
            })
        })
    }

    fn content_frame(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        self.skin.content_frame(group, window, cx)
    }

    fn render_tab_bar(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let tabs = if group
            .panels()
            .iter()
            .all(super::layout::columns::is_empty_editor)
        {
            Empty.into_any_element()
        } else if conversation::matches(group) {
            conversation::render(group, &self.workbench, &self.area, cx)
        } else if group
            .panels()
            .iter()
            .any(|panel| panel.visible(cx) && sidebar::is_navigation(panel))
        {
            div()
                .flex()
                .flex_col()
                .flex_shrink_0()
                .child(sidebar::render_header(
                    group.node(),
                    &self.area,
                    &self.workbench,
                    &self.navigation_headers,
                    cx,
                ))
                .when(
                    group
                        .active_panel()
                        .is_some_and(|panel| !sidebar::is_navigation(panel)),
                    |header| header.child(self.tabs.render(group, &self.area, window, cx)),
                )
                .into_any_element()
        } else if !group.is_zoomed()
            && self.area.upgrade().is_some_and(|area| {
                area.read(cx)
                    .layout(DockPlacement::Bottom)
                    .is_some_and(|tree| tree.find_node(group.node()).is_some())
            })
            && group
                .panels()
                .iter()
                .filter(|panel| panel.visible(cx))
                .all(is_tool_panel)
        {
            Empty.into_any_element()
        } else if group.is_collapsed() {
            self.skin.render_tab_bar(group, window, cx)
        } else {
            self.tabs.render(group, &self.area, window, cx)
        };
        let toolbar = (!group.is_collapsed())
            .then(|| group.active_panel())
            .flatten()
            .filter(|panel| panel.visible(cx))
            .and_then(|panel| panel.view().downcast::<crate::canvas::GraphCanvas>().ok())
            .map(|canvas| canvas.read(cx).toolbar_view());
        if let Some(toolbar) = toolbar {
            // Keep graph chrome beside, rather than inside, the active panel's
            // cache: rebuilding that cache forces every nested cache to refresh.
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .flex_shrink_0()
                .child(tabs)
                .child(
                    toolbar.cached(
                        gpui_kit::StyleRefinement::default()
                            .w_full()
                            .h(HEADER_HEIGHT)
                            .flex_shrink_0(),
                    ),
                )
                .into_any_element()
        } else {
            tabs
        }
    }

    fn render_active_panel(
        &self,
        panel: AnyView,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let content = self.skin.render_active_panel(panel, group, window, cx);
        if conversation::matches(group) && !group.is_zoomed() {
            regions::RegionContent::conversation(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(content)
                    .into_any_element(),
                group.node(),
                self.area.clone(),
                self.workbench.clone(),
                self.measurements.clone(),
            )
            .into_any_element()
        } else {
            content
        }
    }

    fn render_drop_indicator(
        &self,
        indicator: DropIndicator,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.skin.render_drop_indicator(indicator, window, cx)
    }

    fn render_empty(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.skin.render_empty(group, window, cx)
    }
}
