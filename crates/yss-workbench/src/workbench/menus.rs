//! Native workbench menus invoke existing resource and DockArea operations.
mod catalog;
mod commands;
mod editing;
mod help;
mod layout;
pub(super) use commands::MenuCommand;
pub(crate) use commands::bind_keys;
pub(crate) use commands::shortcut_commands;

use super::Workbench;
use gpui_kit::component::dock::{BasePanelView, DockPlacement, PanelId, panel_handle};
use gpui_kit::{App, Context, Menu, Window};
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
        if let Some(editor) = self
            .active_editor_panel(cx)
            .and_then(|panel| super::EditorPanel::from_view(panel.view()))
        {
            editor.save(window, cx);
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
                return self.show_assistant(window, cx);
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
                    .and_then(gpui_kit::WeakEntity::upgrade)?;
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

    pub(super) fn prepare_menus(&mut self, window: &Window, cx: &mut Context<Self>) {
        // Unrelated overlays retain the original editor's menu projection.
        if self.menu_context.is_some() && !self.focus.contains_focused(window, cx) {
            return;
        }
        let (text_focus, edits) = self.edit_menu_state(window, cx);
        let context = MenuContext {
            busy: self.is_closing(cx),
            project: self.project.is_some(),
            editor: self.active_editor_panel(cx).map(|panel| panel.panel_id(cx)),
            text_focus,
            edits,
            can_split: self.editor_split_target(cx).is_some(),
            sidebar_open: self.dock.read(cx).is_dock_open(DockPlacement::Left)
                && (!self.dock.read(cx).is_zoomed()
                    || super::layout::columns::conversation_zoomed(self.dock.read(cx))),
            assistant_open: self
                .dock_panel(WorkbenchPanel::Assistant)
                .is_some_and(|(panel, _)| self.panel_placement(panel.panel_id(cx), cx).is_some()),
        };
        context.install(&mut self.menu_context, &self.menu_bar, window, cx);
    }
}

// A disposable menu projection; DockArea remains the authority for live layout state.
#[derive(PartialEq)]
pub(super) struct MenuContext {
    busy: bool,
    project: bool,
    editor: Option<PanelId>,
    text_focus: Option<gpui_kit::FocusHandle>,
    edits: [bool; 6],
    can_split: bool,
    sidebar_open: bool,
    assistant_open: bool,
}

impl MenuContext {
    fn install(
        self,
        previous: &mut Option<Self>,
        menu_bar: &gpui_kit::Entity<gpui_kit::component::menu::AppMenuBar>,
        window: &Window,
        cx: &mut App,
    ) {
        if previous.as_ref() == Some(&self) {
            return;
        }
        // A popup or accessibility-focused trigger stays inside Workbench's focus tree.
        // Freeze both availability and the original text target until menu dismissal.
        if previous.is_some()
            && window
                .context_stack()
                .iter()
                .any(|context| context.contains("AppMenuBar"))
        {
            return;
        }
        let menus = catalog::application_menus(&self);
        cx.set_menus(catalog::application_menus(&self));
        *previous = Some(self);
        gpui_kit::base::GlobalState::global_mut(cx)
            .set_app_menus(menus.into_iter().map(Menu::owned).collect());
        menu_bar.update(cx, |bar, cx| bar.reload(cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::{
        input::{self, Input, InputState},
        menu::AppMenuBar,
    };
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription,
        TestAppContext, WindowOptions, div, prelude::*, px,
    };

    struct MenuHost {
        input: Entity<InputState>,
        menu_bar: Entity<AppMenuBar>,
        menu_context: Option<MenuContext>,
        focus: FocusHandle,
        _input_observation: Subscription,
    }

    impl Render for MenuHost {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let input_focus = self.input.read(cx).focus_handle(cx);
            let text_focus = input_focus.is_focused(window).then_some(input_focus);
            MenuContext {
                busy: false,
                project: false,
                editor: None,
                text_focus,
                edits: [
                    false,
                    false,
                    false,
                    self.input.read(cx).focus_handle(cx).is_focused(window)
                        && window.is_action_available(&input::Copy, cx),
                    false,
                    false,
                ],
                can_split: false,
                sidebar_open: false,
                assistant_open: false,
            }
            .install(&mut self.menu_context, &self.menu_bar, window, cx);
            div()
                .track_focus(&self.focus)
                .on_action(cx.listener(|view, command: &MenuCommand, window, cx| {
                    if let MenuCommand::Edit(command) = command {
                        command.dispatch_text(
                            view.menu_context
                                .as_ref()
                                .and_then(|context| context.text_focus.as_ref()),
                            &view.focus,
                            window,
                            cx,
                        );
                    }
                }))
                .size_full()
                .flex()
                .flex_col()
                .child(div().h(px(32.)).w(px(420.)).child(self.menu_bar.clone()))
                .child(Input::new(&self.input))
        }
    }

    #[gpui_kit::test]
    fn input_menu_survives_focus_transfer_and_restores_input(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::init(cx);
        });
        let (window, host) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let input = cx.new(|cx| InputState::new(window, cx).default_value("draft"));
                    MenuHost {
                        _input_observation: cx.observe(&input, |_, _, cx| cx.notify()),
                        input,
                        menu_bar: AppMenuBar::new(cx),
                        menu_context: None,
                        focus: cx.focus_handle(),
                    }
                })
            })
            .unwrap()
        });
        let input = cx.update(|cx| host.read(cx).input.clone());
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            input.read(cx).focus_handle(cx).focus(window, cx);
            window.render_frame(cx);
            window.dispatch_action(Box::new(input::SelectAll), cx);
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.is_action_available(&input::Copy, cx));
            window
                .within("app-menu-bar")
                .within(1usize)
                .click("menu", cx);
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .context_stack()
                    .iter()
                    .any(|context| context.contains("AppMenuBar")),
                "opening the menu must not rebuild and dismiss its popup"
            );
            assert!(host.read(cx).focus.contains_focused(window, cx));
            window.press("escape", cx);
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(input.read(cx).focus_handle(cx).is_focused(window));
            // Keyboard/accessibility activation focuses the trigger before opening it.
            for _ in 0..16 {
                window.focus_next(cx);
                window.render_frame(cx);
                if window
                    .within("app-menu-bar")
                    .within(1usize)
                    .find("menu")
                    .focused()
                    == Some(true)
                {
                    break;
                }
            }
            assert_eq!(
                window
                    .within("app-menu-bar")
                    .within(1usize)
                    .find("menu")
                    .focused(),
                Some(true)
            );
            window.press("enter", cx);
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.press("down", cx);
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                cx.read_from_clipboard()
                    .and_then(|item| item.text())
                    .as_deref(),
                Some("draft")
            );
            assert!(input.read(cx).focus_handle(cx).is_focused(window));
        })
        .unwrap();
    }
}
