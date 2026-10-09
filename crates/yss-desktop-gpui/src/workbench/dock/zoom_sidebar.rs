//! Render the existing sidebar panels beside a zoomed conversation from the root topology.
use super::super::{Workbench, layout::columns, sidebar};
use gpui::{AnyElement, App, Empty, IntoElement, RenderOnce, WeakEntity, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, AxisExt,
    dock::{DockArea, DockPlacement, PaneNode, PaneRef, PanelHandle},
    tab::{Tab, TabBar},
};

#[derive(IntoElement)]
pub(super) struct Sidebar {
    pub area: WeakEntity<DockArea>,
    pub workbench: WeakEntity<Workbench>,
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(area) = self.area.upgrade() else {
            return Empty.into_any_element();
        };
        let dock = area.read(cx);
        if !columns::conversation_zoomed(dock) || !dock.is_dock_open(DockPlacement::Left) {
            return Empty.into_any_element();
        }
        let Some(tree) = dock.layout(DockPlacement::Left) else {
            return Empty.into_any_element();
        };
        let node = tree.root().clone();
        let width = dock.dock_size(DockPlacement::Left).unwrap_or_default();
        div()
            .w(width)
            .h_full()
            .flex_shrink_0()
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .child(render_node(&node, &self.area, &self.workbench, window, cx))
            .into_any_element()
    }
}

fn render_node(
    node: &PaneNode,
    area: &WeakEntity<DockArea>,
    workbench: &WeakEntity<Workbench>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match node.kind() {
        PaneRef::Split {
            axis,
            children,
            sizes,
        } => div()
            .size_full()
            .flex()
            .when(axis.is_vertical(), |view| view.flex_col())
            .children(
                children
                    .iter()
                    .enumerate()
                    .map(|(ix, node)| {
                        let content = render_node(node, area, workbench, window, cx);
                        div()
                            .min_h_0()
                            .min_w_0()
                            .when_some(
                                sizes[ix].filter(|_| ix + 1 < children.len()),
                                |view, size| {
                                    if axis.is_horizontal() {
                                        view.w(size).h_full().flex_shrink_0()
                                    } else {
                                        view.h(size).w_full().flex_shrink_0()
                                    }
                                },
                            )
                            .when(ix + 1 == children.len() || sizes[ix].is_none(), |view| {
                                view.flex_1()
                            })
                            .child(content)
                    })
                    .collect::<Vec<_>>(),
            )
            .into_any_element(),
        PaneRef::Tabs {
            panels: ids,
            active_ix,
        } => {
            let Some(entity) = area.upgrade() else {
                return Empty.into_any_element();
            };
            let dock = entity.read(cx);
            let panels = ids
                .iter()
                .filter_map(|id| dock.panel(*id).cloned())
                .filter(|panel| panel.visible(cx))
                .collect::<Vec<_>>();
            let active = ids.get(active_ix).copied();
            let panel = panels
                .iter()
                .find(|panel| Some(panel.panel_id(cx)) == active)
                .or_else(|| panels.first())
                .cloned();
            let navigation = panels.iter().any(sidebar::is_navigation);
            let resource_active = panel
                .as_ref()
                .is_some_and(|panel| !sidebar::is_navigation(panel));
            let mut frame = div().size_full().flex().flex_col().overflow_hidden();
            if navigation {
                frame = frame.child(sidebar::render_header(node.id(), area, workbench, cx));
            }
            if !navigation || resource_active {
                let tabs = panels
                    .iter()
                    .filter_map(|panel| {
                        let handle = PanelHandle::of(panel)?;
                        let id = panel.panel_id(cx);
                        let area = area.clone();
                        Some(Tab::new().child(handle.title(window, cx)).on_click(
                            move |_, window, cx| {
                                let _ =
                                    area.update(cx, |dock, cx| dock.select_panel(id, window, cx));
                            },
                        ))
                    })
                    .collect::<Vec<_>>();
                let selected = panels
                    .iter()
                    .position(|panel| Some(panel.panel_id(cx)) == active)
                    .unwrap_or(0);
                frame = frame.child(
                    TabBar::new(("zoom-sidebar-tabs", node.id().as_u64()))
                        .selected_index(selected)
                        .children(tabs),
                );
            }
            frame
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .when_some(panel, |view, panel| view.child(panel.view())),
                )
                .into_any_element()
        }
    }
}
