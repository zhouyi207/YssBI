//! Menu layout operations mutate the existing DockArea without recreating editor entities.
use super::{super::Workbench, WorkbenchPanel};
use gpui_kit::component::{
    Placement,
    dock::{DockLayout, DockPlacement, InsertTarget, NodeId, PaneRef, PanelId},
};
use gpui_kit::{App, Context, Window, px};

impl Workbench {
    pub(super) fn editor_split_target(&self, cx: &App) -> Option<(PanelId, NodeId)> {
        let panel = self.active_editor_panel(cx)?.panel_id(cx);
        let placement = self.displayed_panel_placement(panel, cx)?;
        let tree = self.dock.read(cx).layout(placement)?;
        let node = tree.find_node(tree.find_panel_node(panel)?)?;
        // DockArea normalizes empty groups. Moving the only tab cannot produce a split.
        matches!(node.kind(), PaneRef::Tabs { panels, .. } if panels.len() > 1)
            .then_some((panel, node.id()))
    }

    pub(super) fn split_editor(
        &mut self,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor_command_panel(window, cx).is_none() {
            return;
        }
        let Some((panel, node)) = self.editor_split_target(cx) else {
            return;
        };
        self.dock.update(cx, |dock, cx| {
            dock.set_zoomed_out(window, cx);
            dock.move_panel(
                panel,
                InsertTarget::Split {
                    node,
                    placement,
                    size: None,
                },
                window,
                cx,
            );
            dock.select_panel(panel, window, cx);
            if let Some(panel) = dock.panel(panel) {
                window.focus(&panel.focus_handle(cx), cx);
            }
        });
    }

    pub(super) fn reset_workbench_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.is_none() {
            return;
        }
        let defaults = &crate::preferences::current(cx).workspace;
        let show_details = defaults.show_details;
        let show_bottom = defaults.show_bottom_panel;
        let active = self.active_editor_panel(cx);
        let dock = self.dock.read(cx);
        let mut panels = [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .filter_map(|placement| dock.layout(placement))
        .flat_map(|tree| tree.panels())
        .filter_map(|id| dock.panel(id).cloned())
        .collect::<Vec<_>>();
        let sidebars = [
            (
                DockPlacement::Left,
                240.,
                true,
                vec![
                    WorkbenchPanel::Project,
                    WorkbenchPanel::Nodes,
                    WorkbenchPanel::Assistant,
                    WorkbenchPanel::Plugins,
                ],
            ),
            (
                DockPlacement::Right,
                300.,
                show_details,
                vec![WorkbenchPanel::Details],
            ),
            (
                DockPlacement::Bottom,
                220.,
                show_bottom,
                vec![
                    WorkbenchPanel::Problems,
                    WorkbenchPanel::Output,
                    WorkbenchPanel::Results,
                    WorkbenchPanel::Logs,
                ],
            ),
        ]
        .map(|(placement, width, open, kinds)| {
            let views = kinds
                .into_iter()
                .filter_map(|kind| self.dock_panel(kind).map(|(panel, _)| panel))
                .collect::<Vec<_>>();
            for panel in &views {
                if !panels
                    .iter()
                    .any(|held| held.panel_id(cx) == panel.panel_id(cx))
                {
                    panels.push(panel.clone());
                }
            }
            (placement, width, open, views)
        });
        let center = panels
            .into_iter()
            .fold(DockLayout::tabs(), |layout, panel| {
                layout.panel_view(panel, cx)
            });
        self.dock.update(cx, |dock, cx| {
            dock.set_zoomed_out(window, cx);
            // Keep every live panel registered throughout rearrangement: on_removed would
            // discard drafts or release result leases if resetting closed/reopened tabs.
            dock.set_center(center, window, cx);
            for (placement, _, _, _) in &sidebars {
                dock.remove_dock(*placement, window, cx);
            }
            for (placement, width, open, panels) in sidebars {
                if panels.is_empty() {
                    continue;
                }
                dock.set_dock(placement, DockLayout::tabs(), window, cx);
                for panel in &panels {
                    let node = dock
                        .layout(placement)
                        .expect("just installed dock")
                        .root()
                        .id();
                    dock.move_panel(
                        panel.panel_id(cx),
                        InsertTarget::Tabs {
                            node,
                            ix: None,
                            activate: false,
                        },
                        window,
                        cx,
                    );
                }
                dock.set_dock_size(placement, px(width), window, cx);
                if dock.is_dock_open(placement) != open {
                    dock.toggle_dock(placement, window, cx);
                }
            }
            super::super::layout::columns::reset(dock, window, cx);
            if let Some(panel) = active {
                dock.select_panel(panel.panel_id(cx), window, cx);
                window.focus(&panel.focus_handle(cx), cx);
            }
        });
    }
}
