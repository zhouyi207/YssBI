//! Menu actions route to existing workbench commands after checking current availability.
use super::{super::Workbench, WorkbenchPanel, editing::EditCommand, help::HelpPage};
use crate::{
    bindings::primary,
    canvas::SaveGraph,
    workbench::{
        SaveAllGraphs, ShowSettings,
        dock::{CloseAllTabs, CloseOtherTabs, CloseTab},
    },
};
use gpui_kit::component::{Placement, dock::DockPlacement};
use gpui_kit::{Action, App, Context, KeyBinding, Window};
use yss_graph_document::GraphResourceKind;
use yss_settings::ThemeMode;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::workbench) enum DatasetFormat {
    Csv,
    Parquet,
}

impl DatasetFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Parquet => "parquet",
        }
    }
}

#[derive(Clone, PartialEq, gpui_kit::Action)]
#[action(namespace = native_workbench, no_json)]
pub(in crate::workbench) enum MenuCommand {
    NewProject,
    SaveProjectAs,
    CloseProject,
    ImportData,
    ExportDataset(DatasetFormat),
    NewGraph(GraphResourceKind),
    NewDocument,
    NewMind,
    NewChart,
    ShowPanel(WorkbenchPanel),
    ToggleDock(DockPlacement),
    ToggleZoom,
    Theme(ThemeMode),
    Edit(EditCommand),
    InspectResults,
    ClearRunResults,
    ResetLayout,
    SplitEditor(Placement),
    ShowKeybindings,
    Help(HelpPage),
    About,
    Exit,
}

pub(crate) fn bind_keys(cx: &mut App) {
    let workbench = |keys, action| primary(keys, action, "Workbench");
    cx.bind_keys(
        [
            primary("s", SaveGraph, "Workbench"),
            primary("shift-s", SaveAllGraphs, "Workbench"),
            primary(",", ShowSettings, "Workbench"),
            primary("w", CloseTab, "Workbench"),
            workbench("n", MenuCommand::NewGraph(GraphResourceKind::EventGraph)),
            workbench("q", MenuCommand::Exit),
            workbench("z", MenuCommand::Edit(EditCommand::Undo)),
            workbench("shift-z", MenuCommand::Edit(EditCommand::Redo)),
            workbench("y", MenuCommand::Edit(EditCommand::Redo)),
            workbench("x", MenuCommand::Edit(EditCommand::Cut)),
            workbench("c", MenuCommand::Edit(EditCommand::Copy)),
            workbench("v", MenuCommand::Edit(EditCommand::Paste)),
            workbench("a", MenuCommand::Edit(EditCommand::SelectAll)),
        ]
        .into_iter()
        .flatten(),
    );
    cx.bind_keys([KeyBinding::new(
        "delete",
        MenuCommand::Edit(EditCommand::Delete),
        Some("Workbench"),
    )]);
}

/// Explicit IDs distinguish parameterized `no_json` actions without accepting arbitrary JSON.
pub(crate) fn shortcut_commands() -> Vec<crate::keymap::ShortcutCommand> {
    use crate::keymap::ShortcutCommand;
    let menu_commands = [
        (
            "NewProject",
            "native.workbench.newProject",
            MenuCommand::NewProject,
        ),
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
            "ExportCsv",
            "menubar.exportCsv",
            MenuCommand::ExportDataset(DatasetFormat::Csv),
        ),
        (
            "ExportParquet",
            "menubar.exportParquet",
            MenuCommand::ExportDataset(DatasetFormat::Parquet),
        ),
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
            "ShowProject",
            "activityBar.project",
            MenuCommand::ShowPanel(WorkbenchPanel::Project),
        ),
        (
            "ShowNodes",
            "activityBar.nodes",
            MenuCommand::ShowPanel(WorkbenchPanel::Nodes),
        ),
        (
            "ShowAssistant",
            "panel.assistant",
            MenuCommand::ShowPanel(WorkbenchPanel::Assistant),
        ),
        (
            "ShowPlugins",
            "activityBar.plugins",
            MenuCommand::ShowPanel(WorkbenchPanel::Plugins),
        ),
        (
            "ShowProperties",
            "native.workbench.properties",
            MenuCommand::ShowPanel(WorkbenchPanel::Details),
        ),
        (
            "ShowProblems",
            "panel.problems",
            MenuCommand::ShowPanel(WorkbenchPanel::Problems),
        ),
        (
            "ShowOutput",
            "panel.output",
            MenuCommand::ShowPanel(WorkbenchPanel::Output),
        ),
        (
            "ShowResults",
            "panel.results",
            MenuCommand::ShowPanel(WorkbenchPanel::Results),
        ),
        (
            "ShowLogs",
            "panel.logs",
            MenuCommand::ShowPanel(WorkbenchPanel::Logs),
        ),
        (
            "ToggleSidebar",
            "menubar.leftSidebar",
            MenuCommand::ToggleDock(DockPlacement::Left),
        ),
        (
            "ToggleRightSidebar",
            "menubar.rightSidebar",
            MenuCommand::ToggleDock(DockPlacement::Right),
        ),
        (
            "ToggleBottomPanel",
            "menubar.bottomPanel",
            MenuCommand::ToggleDock(DockPlacement::Bottom),
        ),
        ("ToggleZoom", "menubar.zoomPanel", MenuCommand::ToggleZoom),
        (
            "LightTheme",
            "preferences.light",
            MenuCommand::Theme(ThemeMode::Light),
        ),
        (
            "DarkTheme",
            "preferences.dark",
            MenuCommand::Theme(ThemeMode::Dark),
        ),
        (
            "SystemTheme",
            "preferences.system",
            MenuCommand::Theme(ThemeMode::System),
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
            "SelectAll",
            "menubar.selectAll",
            MenuCommand::Edit(EditCommand::SelectAll),
        ),
        (
            "InspectResults",
            "menubar.viewResults",
            MenuCommand::InspectResults,
        ),
        (
            "ClearRunResults",
            "canvas.clearExecutionArtifacts",
            MenuCommand::ClearRunResults,
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
            "ShowKeybindings",
            "menubar.keyboardShortcuts",
            MenuCommand::ShowKeybindings,
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
            "ReportIssue",
            "menubar.reportIssue",
            MenuCommand::Help(HelpPage::ReportIssue),
        ),
        ("About", "menubar.about", MenuCommand::About),
        ("Exit", "native.workbench.quit", MenuCommand::Exit),
    ]
    .into_iter()
    .map(|(id, label, action)| {
        (
            format!("native_workbench::MenuCommand::{id}"),
            label,
            Box::new(action) as Box<dyn Action>,
        )
    });
    let tab_commands = [
        ("menubar.closeTab", CloseTab.boxed_clone()),
        (
            "native.workbench.closeOtherTabs",
            CloseOtherTabs.boxed_clone(),
        ),
        ("native.workbench.closeAllTabs", CloseAllTabs.boxed_clone()),
    ]
    .into_iter()
    .map(|(label, action)| (action.name().to_string(), label, action));
    menu_commands
        .chain(tab_commands)
        .map(|(id, label, action)| ShortcutCommand::new(id, label, "Workbench", action))
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
            MenuCommand::NewProject => {
                self.project_form(crate::projects::form::ProjectFormKind::Create, window, cx)
            }
            MenuCommand::SaveProjectAs => {
                self.project_form(crate::projects::form::ProjectFormKind::SaveAs, window, cx)
            }
            MenuCommand::CloseProject => self.close_project(window, cx),
            MenuCommand::ImportData => self.import_dialog(window, cx),
            MenuCommand::ExportDataset(format) => {
                if let Some(editor) = self.editor_command_panel(window, cx).and_then(|panel| {
                    panel
                        .view()
                        .downcast::<crate::databases::DatabaseEditor>()
                        .ok()
                }) {
                    editor.update(cx, |editor, cx| {
                        editor.export(format.extension(), window, cx)
                    });
                }
            }
            MenuCommand::NewGraph(kind) => self.create_graph(*kind, window, cx),
            MenuCommand::NewDocument => {
                self.create_file_dialog(super::super::files::FileKind::Document, window, cx)
            }
            MenuCommand::NewMind => {
                self.create_file_dialog(super::super::files::FileKind::Mind, window, cx)
            }
            MenuCommand::NewChart => {
                self.create_file_dialog(super::super::files::FileKind::Chart, window, cx)
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
            MenuCommand::ToggleDock(placement) => {
                self.toggle_dock_region(*placement, window, cx);
            }
            MenuCommand::ToggleZoom => self.toggle_zoom(window, cx),
            MenuCommand::Theme(mode) => {
                let mode = *mode;
                self.settings
                    .update(cx, |settings, cx| settings.set_theme_mode(mode, cx));
            }
            MenuCommand::Edit(command) => self.edit_current(*command, window, cx),
            MenuCommand::InspectResults => {
                if let Some(graph) = self.command_graph(window, cx) {
                    graph.update(cx, |graph, cx| graph.inspect_results(cx));
                }
            }
            MenuCommand::ClearRunResults => {
                if let Some(graph) = self.command_graph(window, cx) {
                    graph.update(cx, |graph, cx| {
                        if let Some(run) = graph.clearable_run().cloned() {
                            graph.clear_run_notice(&run, cx);
                        }
                    });
                }
            }
            MenuCommand::ResetLayout => self.reset_workbench_layout(window, cx),
            MenuCommand::SplitEditor(placement) => self.split_editor(*placement, window, cx),
            MenuCommand::ShowKeybindings => {
                self.settings
                    .update(cx, |settings, cx| settings.show_keybindings(cx));
                self.show_settings(window, cx);
            }
            MenuCommand::Help(page) => cx.open_url(&page.url()),
            MenuCommand::About => super::help::show_about(window, cx),
            MenuCommand::Exit => self.request_close(window, cx),
        }
    }

    fn command_graph(
        &self,
        window: &Window,
        cx: &App,
    ) -> Option<gpui_kit::Entity<crate::canvas::GraphCanvas>> {
        self.editor_command_panel(window, cx)?
            .view()
            .downcast()
            .ok()
    }
}
