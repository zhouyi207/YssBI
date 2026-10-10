//! Native workbench menus invoke existing resource and DockArea operations.
mod catalog;
mod commands;
mod editing;
mod help;
mod layout;
pub(super) use commands::MenuCommand;
pub(crate) use commands::bind_keys;

use super::Workbench;
use crate::canvas::GraphCommand;
use gpui::{App, Context, Menu, Window};
use gpui_component::dock::{BasePanelView, DockPlacement, PanelId, panel_handle};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum WorkbenchPanel {
    Project,
    Nodes,
    Details,
    Problems,
    Output,
    Results,
    Logs,
    Plugins,
    Settings,
    Assistant,
}

impl Workbench {
    pub(super) fn save_current(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_closing(cx) || self.editor_command_panel(window, cx).is_none() {
            return;
        }
        if let Some(document) = self.details.read(cx).document() {
            document.update(cx, |document, cx| document.save(window, cx));
        } else if let Some(mind) = self.details.read(cx).mind() {
            mind.update(cx, |mind, cx| mind.save(window, cx));
        } else if let Some(editor) = self.details.read(cx).database() {
            editor.update(cx, |editor, cx| editor.save(window, cx));
        } else if let Some(chart) = self.details.read(cx).chart() {
            chart.update(cx, |chart, cx| chart.save(window, cx));
        } else if let Some(graph) = self.details.read(cx).graph() {
            graph.update(cx, |graph, cx| graph.submit(GraphCommand::Save, None, cx));
        }
    }

    pub(super) fn show_panel(
        &mut self,
        panel: WorkbenchPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match panel {
            WorkbenchPanel::Assistant => {
                self.show_assistant(window, cx);
                return true;
            }
            WorkbenchPanel::Settings => {
                self.show_settings(window, cx);
                return true;
            }
            WorkbenchPanel::Plugins => {
                self.show_plugins(window, cx);
                return true;
            }
            _ => {}
        }
        let Some((panel, placement)) = self.dock_panel(panel) else {
            return false;
        };
        self.present_panel(panel, placement, window, cx);
        true
    }

    pub(super) fn dock_panel(
        &self,
        panel: WorkbenchPanel,
    ) -> Option<(Arc<dyn BasePanelView>, DockPlacement)> {
        Some(match panel {
            WorkbenchPanel::Project | WorkbenchPanel::Nodes | WorkbenchPanel::Assistant => {
                let key = match panel {
                    WorkbenchPanel::Project => "project",
                    WorkbenchPanel::Nodes => "nodes",
                    _ => "assistant",
                };
                let panel = self
                    .activities
                    .get(key)
                    .and_then(gpui::WeakEntity::upgrade)?;
                (panel_handle(panel), DockPlacement::Left)
            }
            WorkbenchPanel::Details => (panel_handle(self.details.clone()), DockPlacement::Right),
            WorkbenchPanel::Problems => {
                (panel_handle(self.problems.clone()), DockPlacement::Bottom)
            }
            WorkbenchPanel::Output => (panel_handle(self.output.clone()), DockPlacement::Bottom),
            WorkbenchPanel::Results => (panel_handle(self.results.clone()), DockPlacement::Bottom),
            WorkbenchPanel::Logs => (panel_handle(self.logs.clone()), DockPlacement::Bottom),
            WorkbenchPanel::Plugins => (
                panel_handle(self.plugins_sidebar.clone()),
                DockPlacement::Left,
            ),
            WorkbenchPanel::Settings => {
                return None;
            }
        })
    }

    pub(super) fn panel_is_displayed(&self, panel: WorkbenchPanel, cx: &App) -> bool {
        self.dock_panel(panel).is_some_and(|(panel, _)| {
            self.displayed_panel_placement(panel.panel_id(cx), cx)
                .is_some()
        })
    }

    pub(super) fn toggle_panel(
        &mut self,
        panel: WorkbenchPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            return;
        }
        if self.dock.read(cx).is_zoomed() {
            self.dock
                .update(cx, |dock, cx| dock.set_zoomed_out(window, cx));
            self.show_panel(panel, window, cx);
            return;
        }
        let displayed = self
            .dock_panel(panel)
            .and_then(|(panel, _)| self.displayed_panel_placement(panel.panel_id(cx), cx));
        if let Some(placement) = displayed.filter(|place| *place != DockPlacement::Center) {
            self.dock
                .update(cx, |dock, cx| dock.toggle_dock(placement, window, cx));
        } else {
            self.show_panel(panel, window, cx);
        }
    }

    pub(super) fn prepare_menus(&mut self, cx: &mut Context<Self>) {
        let context = MenuContext {
            busy: self.is_closing(cx),
            project: self.project.is_some(),
            editor: self.active_editor_panel(cx).map(|panel| panel.panel_id(cx)),
            edits: self.edit_menu_availability(cx),
            can_split: self.editor_split_target(cx).is_some(),
            sidebar_open: self.dock.read(cx).is_dock_open(DockPlacement::Left)
                && (!self.dock.read(cx).is_zoomed()
                    || super::layout::columns::conversation_zoomed(self.dock.read(cx))),
            assistant_open: self
                .dock_panel(WorkbenchPanel::Assistant)
                .is_some_and(|(panel, _)| self.panel_placement(panel.panel_id(cx), cx).is_some()),
        };
        if self.menu_context == Some(context) {
            return;
        }
        self.menu_context = Some(context);
        let menus = catalog::application_menus(context);
        cx.set_menus(catalog::application_menus(context));
        gpui_base::GlobalState::global_mut(cx)
            .set_app_menus(menus.into_iter().map(Menu::owned).collect());
        self.menu_bar.update(cx, |bar, cx| bar.reload(cx));
    }
}

// A disposable menu projection; DockArea remains the authority for live layout state.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct MenuContext {
    busy: bool,
    project: bool,
    editor: Option<PanelId>,
    edits: [bool; 6],
    can_split: bool,
    sidebar_open: bool,
    assistant_open: bool,
}
