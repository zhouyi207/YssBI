//! Menu actions route to existing workbench commands after checking current availability.
use super::{super::Workbench, WorkbenchPanel, editing::EditCommand, help::HelpPage};
use crate::{canvas::SaveGraph, workbench::ShowSettings};
use gpui_kit::component::{Placement, dock::DockPlacement};
use gpui_kit::{App, Context, KeyBinding, Window};
use yss_graph_document::GraphResourceKind;

#[derive(Clone, PartialEq, gpui_kit::Action)]
#[action(namespace = native_workbench, no_json)]
pub(in crate::workbench) enum MenuCommand {
    SaveProjectAs,
    CloseProject,
    ImportData,
    NewGraph(GraphResourceKind),
    NewDocument,
    NewMind,
    NewChart,
    ShowPanel(WorkbenchPanel),
    ToggleSidebar,
    Edit(EditCommand),
    ResetLayout,
    SplitEditor(Placement),
    OpenLogsWindow,
    Help(HelpPage),
    About,
    Exit,
}

pub(crate) fn bind_keys(cx: &mut App) {
    for modifier in if cfg!(target_os = "macos") {
        ["ctrl", "cmd"]
    } else {
        ["cmd", "ctrl"]
    } {
        cx.bind_keys([
            KeyBinding::new(&format!("{modifier}-s"), SaveGraph, Some("Workbench")),
            KeyBinding::new(&format!("{modifier}-,"), ShowSettings, Some("Workbench")),
        ]);
        for (key, command) in [
            ("n", MenuCommand::NewGraph(GraphResourceKind::EventGraph)),
            ("shift-s", MenuCommand::SaveProjectAs),
            ("z", MenuCommand::Edit(EditCommand::Undo)),
            ("y", MenuCommand::Edit(EditCommand::Redo)),
            ("x", MenuCommand::Edit(EditCommand::Cut)),
            ("c", MenuCommand::Edit(EditCommand::Copy)),
            ("v", MenuCommand::Edit(EditCommand::Paste)),
        ] {
            cx.bind_keys([KeyBinding::new(
                &format!("{modifier}-{key}"),
                command,
                Some("Workbench"),
            )]);
        }
    }
    cx.bind_keys([KeyBinding::new(
        "delete",
        MenuCommand::Edit(EditCommand::Delete),
        Some("Workbench"),
    )]);
}

/// Explicit IDs distinguish parameterized `no_json` actions without accepting arbitrary JSON.
pub(crate) fn shortcut_commands() -> Vec<crate::keymap::ShortcutCommand> {
    use crate::keymap::ShortcutCommand;
    [
        (
            "SaveProjectAs",
            "menubar.saveProjectAs",
            MenuCommand::SaveProjectAs,
        ),
        (
            "CloseProject",
            "menubar.closeProject",
            MenuCommand::CloseProject,
        ),
        ("ImportData", "menubar.importData", MenuCommand::ImportData),
        (
            "NewEventGraph",
            "menubar.newEventGraph",
            MenuCommand::NewGraph(GraphResourceKind::EventGraph),
        ),
        (
            "NewFunctionGraph",
            "menubar.newFunctionGraph",
            MenuCommand::NewGraph(GraphResourceKind::FunctionGraph),
        ),
        ("NewDocument", "documents.newDoc", MenuCommand::NewDocument),
        ("NewMind", "documents.newMind", MenuCommand::NewMind),
        ("NewChart", "menubar.newChart", MenuCommand::NewChart),
        (
            "ToggleSidebar",
            "panel.primarySideBar",
            MenuCommand::ToggleSidebar,
        ),
        (
            "ShowAssistant",
            "panel.assistant",
            MenuCommand::ShowPanel(WorkbenchPanel::Assistant),
        ),
        ("Undo", "common.undo", MenuCommand::Edit(EditCommand::Undo)),
        ("Redo", "common.redo", MenuCommand::Edit(EditCommand::Redo)),
        ("Cut", "menubar.cut", MenuCommand::Edit(EditCommand::Cut)),
        ("Copy", "menubar.copy", MenuCommand::Edit(EditCommand::Copy)),
        (
            "Paste",
            "menubar.paste",
            MenuCommand::Edit(EditCommand::Paste),
        ),
        (
            "Delete",
            "common.delete",
            MenuCommand::Edit(EditCommand::Delete),
        ),
        (
            "ResetLayout",
            "menubar.resetLayout",
            MenuCommand::ResetLayout,
        ),
        (
            "SplitEditorRight",
            "menubar.splitEditorRight",
            MenuCommand::SplitEditor(Placement::Right),
        ),
        (
            "SplitEditorDown",
            "menubar.splitEditorDown",
            MenuCommand::SplitEditor(Placement::Bottom),
        ),
        (
            "OpenLogsWindow",
            "menubar.openLogsInNewWindow",
            MenuCommand::OpenLogsWindow,
        ),
        (
            "Architecture",
            "menubar.architecture",
            MenuCommand::Help(HelpPage::Architecture),
        ),
        (
            "Documentation",
            "menubar.documentation",
            MenuCommand::Help(HelpPage::Documentation),
        ),
        (
            "ReleaseNotes",
            "menubar.releaseNotes",
            MenuCommand::Help(HelpPage::ReleaseNotes),
        ),
        (
            "Repository",
            "menubar.githubRepository",
            MenuCommand::Help(HelpPage::Repository),
        ),
        (
            "ReportIssue",
            "menubar.reportIssue",
            MenuCommand::Help(HelpPage::ReportIssue),
        ),
        ("About", "menubar.about", MenuCommand::About),
        ("Exit", "native.workbench.quit", MenuCommand::Exit),
    ]
    .into_iter()
    .map(|(id, label, action)| {
        ShortcutCommand::new(
            format!("native_workbench::MenuCommand::{id}"),
            label,
            "Workbench",
            Box::new(action),
        )
    })
    .collect()
}

impl Workbench {
    pub(in crate::workbench) fn dispatch_menu(
        &mut self,
        command: &MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // An open menu may outlive the state from which its disabled flags were derived.
        if self.is_closing(cx) {
            return;
        }
        match command {
            MenuCommand::SaveProjectAs => {
                self.project_form(crate::projects::form::ProjectFormKind::SaveAs, window, cx)
            }
            MenuCommand::CloseProject => self.close_project(window, cx),
            MenuCommand::ImportData => self.import_dialog(window, cx),
            MenuCommand::NewGraph(kind) => self.create_graph(*kind, window, cx),
            MenuCommand::NewDocument => {
                self.create_file_dialog(super::super::resources::AuthoredKind::Document, window, cx)
            }
            MenuCommand::NewMind => {
                self.create_file_dialog(super::super::resources::AuthoredKind::Mind, window, cx)
            }
            MenuCommand::NewChart => {
                self.create_file_dialog(super::super::resources::AuthoredKind::Chart, window, cx)
            }
            MenuCommand::ShowPanel(panel) => {
                let keep_conversation =
                    super::super::layout::columns::conversation_zoomed(self.dock.read(cx))
                        && self
                            .dock_panel(*panel)
                            .is_some_and(|(panel, _)| super::super::sidebar::is_navigation(&panel));
                if !keep_conversation {
                    self.dock
                        .update(cx, |dock, cx| dock.set_zoomed_out(window, cx));
                }
                self.show_panel(*panel, window, cx);
            }
            MenuCommand::ToggleSidebar => {
                self.toggle_dock_region(DockPlacement::Left, window, cx);
            }
            MenuCommand::Edit(command) => self.edit_current(*command, window, cx),
            MenuCommand::ResetLayout => self.reset_workbench_layout(window, cx),
            MenuCommand::SplitEditor(placement) => self.split_editor(*placement, window, cx),
            MenuCommand::OpenLogsWindow => self.show_logs_window(window, cx),
            MenuCommand::Help(page) => cx.open_url(&page.url()),
            MenuCommand::About => super::help::show_about(window, cx),
            MenuCommand::Exit => self.request_close(window, cx),
        }
    }
}
