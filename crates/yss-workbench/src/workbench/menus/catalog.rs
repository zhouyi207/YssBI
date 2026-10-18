//! Menus index existing commands: each item dispatches the action its toolbar, context menu
//! or shortcut uses, and stays in place while disabled.
use super::{
    EditorKind, MenuCommand, MenuContext, WorkbenchPanel, editing::EditCommand, help::HelpPage,
};
use crate::{
    canvas::{self, SaveGraph},
    documents::ToggleDocumentPreview,
    minds::{FitMind, FitTopics},
    text::translate,
    workbench::{
        OpenProjectDirectory, OpenRecentProject, SaveAllGraphs, ShowSettings,
        dock::{CloseAllTabs, CloseOtherTabs, CloseTab},
    },
};
use gpui_kit::component::{Placement, dock::DockPlacement};
use gpui_kit::{Action, Menu, MenuItem, SystemMenuType};
use yss_database_contract::DatabaseExportFormat;
use yss_graph_document::GraphResourceKind;
use yss_settings::ThemeMode;

fn item(key: &str, action: impl Action, disabled: bool) -> MenuItem {
    boxed_item(key, Box::new(action), disabled)
}

fn boxed_item(key: &str, action: Box<dyn Action>, disabled: bool) -> MenuItem {
    MenuItem::Action {
        name: translate(key).into(),
        action,
        os_action: None,
        checked: false,
        disabled,
    }
}

fn submenu(key: &str, items: impl IntoIterator<Item = MenuItem>) -> MenuItem {
    MenuItem::submenu(Menu::new(translate(key)).items(items))
}

pub(super) fn application_menus(context: &MenuContext) -> Vec<Menu> {
    let mut menus = vec![];
    if cfg!(target_os = "macos") {
        menus.push(Menu::new("YssBI").items([
            item("menubar.about", MenuCommand::About, context.busy),
            MenuItem::separator(),
            item("menubar.settings", ShowSettings, context.busy),
            MenuItem::separator(),
            MenuItem::os_submenu(translate("menubar.services"), SystemMenuType::Services),
            MenuItem::separator(),
            item("native.workbench.quit", MenuCommand::Exit, context.busy),
        ]));
    }
    menus.extend([
        file_menu(context),
        edit_menu(context),
        view_menu(context),
        run_menu(context),
        data_menu(context),
        window_menu(context),
        help_menu(context.busy),
    ]);
    menus
}

fn file_menu(context: &MenuContext) -> Menu {
    let disabled = context.busy || !context.project;
    let mut items = vec![
        submenu(
            "menubar.new",
            [
                item(
                    "menubar.eventGraph",
                    MenuCommand::NewGraph(GraphResourceKind::EventGraph),
                    disabled,
                ),
                item(
                    "menubar.functionGraph",
                    MenuCommand::NewGraph(GraphResourceKind::FunctionGraph),
                    disabled,
                ),
                item("menubar.chart", MenuCommand::NewChart, disabled),
                item("menubar.mind", MenuCommand::NewMind, disabled),
                item("menubar.document", MenuCommand::NewDocument, disabled),
            ],
        ),
        item(
            "native.workbench.newProject",
            MenuCommand::NewProject,
            context.busy,
        ),
        item("menubar.openProject", OpenProjectDirectory, context.busy),
        item(
            "native.workbench.openRecentProjects",
            OpenRecentProject,
            context.busy,
        ),
        item("menubar.closeProject", MenuCommand::CloseProject, disabled),
        MenuItem::separator(),
        item(
            "common.save",
            SaveGraph,
            disabled || context.editor.is_none(),
        ),
        item(
            "menubar.saveAll",
            SaveAllGraphs,
            disabled || !context.unsaved,
        ),
        item(
            "menubar.saveProjectAs",
            MenuCommand::SaveProjectAs,
            disabled,
        ),
    ];
    if !cfg!(target_os = "macos") {
        items.extend([
            MenuItem::separator(),
            item("menubar.settings", ShowSettings, context.busy),
            item("menubar.exit", MenuCommand::Exit, context.busy),
        ]);
    }
    Menu::new(translate("menubar.file")).items(items)
}

fn edit_menu(context: &MenuContext) -> Menu {
    // A focused editor receives its own actions directly; any other focus goes through the
    // workbench, which routes to the active editor or the focused text field.
    let editor = context
        .commands
        .kind
        .filter(|_| context.editor_focused && context.text_focus.is_none());
    let edit = |key: &str, command: EditCommand| {
        let enabled = EditCommand::ALL
            .iter()
            .position(|candidate| *candidate == command)
            .is_some_and(|ix| context.edits[ix]);
        let disabled = context.busy || !enabled;
        match editor.and_then(|kind| command.editor_action(kind)) {
            Some(action) => boxed_item(key, action, disabled),
            None => item(key, MenuCommand::Edit(command), disabled),
        }
    };
    let graph = &context.commands.graph;
    let graph_disabled = context.busy || context.commands.kind != Some(EditorKind::Graph);
    Menu::new(translate("menubar.edit")).items([
        edit("common.undo", EditCommand::Undo),
        edit("common.redo", EditCommand::Redo),
        MenuItem::separator(),
        edit("menubar.cut", EditCommand::Cut),
        edit("menubar.copy", EditCommand::Copy),
        edit("menubar.paste", EditCommand::Paste),
        item(
            "contextMenu.node.duplicate",
            canvas::DuplicateSelection,
            graph_disabled || !graph.duplicate,
        ),
        edit("common.delete", EditCommand::Delete),
        MenuItem::separator(),
        edit("menubar.selectAll", EditCommand::SelectAll),
        item(
            "native.canvas.addNode",
            canvas::OpenNodePalette,
            graph_disabled || !graph.editable,
        ),
    ])
}

fn view_menu(context: &MenuContext) -> Menu {
    let disabled = context.busy || !context.project;
    let panel = |key: &str, panel: WorkbenchPanel| {
        item(key, MenuCommand::ShowPanel(panel), disabled).checked(context.panels.contains(&panel))
    };
    let dock = |key: &str, ix: usize, placement: DockPlacement| {
        let (available, open) = context.docks[ix];
        item(
            key,
            MenuCommand::ToggleDock(placement),
            disabled || !available,
        )
        .checked(context.project && open)
    };
    let kind = context.commands.kind;
    let mind = kind == Some(EditorKind::Mind);
    let graph = kind == Some(EditorKind::Graph);
    let fit_disabled = context.busy || !(graph || mind);
    let fit = if mind {
        item("menubar.fitCanvas", FitMind, fit_disabled)
    } else {
        item("menubar.fitCanvas", canvas::FrameGraph, fit_disabled)
    };
    let selection_disabled =
        context.busy || !(mind || graph && context.commands.graph.has_selection);
    let focus_selection = if mind {
        item("menubar.focusSelection", FitTopics, selection_disabled)
    } else {
        item(
            "menubar.focusSelection",
            canvas::FrameSelection,
            selection_disabled,
        )
    };
    let theme = |key: &str, mode: ThemeMode| {
        item(key, MenuCommand::Theme(mode), context.busy).checked(context.theme == mode)
    };
    Menu::new(translate("menubar.view")).items([
        panel("activityBar.project", WorkbenchPanel::Project),
        panel("activityBar.nodes", WorkbenchPanel::Nodes),
        panel("panel.assistant", WorkbenchPanel::Assistant),
        panel("activityBar.plugins", WorkbenchPanel::Plugins),
        MenuItem::separator(),
        panel("native.workbench.properties", WorkbenchPanel::Details),
        panel("panel.problems", WorkbenchPanel::Problems),
        panel("panel.output", WorkbenchPanel::Output),
        panel("panel.results", WorkbenchPanel::Results),
        panel("panel.logs", WorkbenchPanel::Logs),
        MenuItem::separator(),
        dock("menubar.leftSidebar", 0, DockPlacement::Left),
        dock("menubar.rightSidebar", 1, DockPlacement::Right),
        dock("menubar.bottomPanel", 2, DockPlacement::Bottom),
        item("menubar.zoomPanel", MenuCommand::ToggleZoom, disabled).checked(context.zoomed),
        MenuItem::separator(),
        fit,
        focus_selection,
        item(
            "menubar.documentPreview",
            ToggleDocumentPreview,
            context.busy || kind != Some(EditorKind::Document),
        ),
        MenuItem::separator(),
        submenu(
            "preferences.appearance.theme",
            [
                theme("preferences.light", ThemeMode::Light),
                theme("preferences.dark", ThemeMode::Dark),
                theme("preferences.system", ThemeMode::System),
            ],
        ),
        item("menubar.resetLayout", MenuCommand::ResetLayout, disabled),
    ])
}

fn run_menu(context: &MenuContext) -> Menu {
    let graph = &context.commands.graph;
    let disabled = context.busy || context.commands.kind != Some(EditorKind::Graph);
    let event_graph = !disabled && graph.event_graph;
    Menu::new(translate("menubar.run")).items([
        item(
            "menubar.runGraph",
            canvas::RunWholeGraph,
            disabled || graph.running || !graph.runnable,
        ),
        item(
            "menubar.stopGraph",
            canvas::CancelRun,
            disabled || !graph.running,
        ),
        item(
            "menubar.runCurrentNode",
            canvas::RunCurrentNode,
            !event_graph || !graph.run_node,
        ),
        item(
            "menubar.runToCurrentNode",
            canvas::RunToNode,
            !event_graph || !graph.run_node,
        ),
        MenuItem::separator(),
        item(
            "menubar.viewResults",
            MenuCommand::InspectResults,
            disabled || !graph.inspect,
        ),
        item(
            "canvas.clearExecutionArtifacts",
            MenuCommand::ClearRunResults,
            disabled || !graph.clear_run,
        ),
        item(
            "native.canvas.refreshRun",
            canvas::RefreshRunState,
            disabled,
        ),
    ])
}

fn data_menu(context: &MenuContext) -> Menu {
    let export_disabled = context.busy || !context.commands.export;
    Menu::new(translate("menubar.data")).items([
        item(
            "menubar.importData",
            MenuCommand::ImportData,
            context.busy || !context.project,
        ),
        submenu(
            "menubar.exportDataset",
            [
                item(
                    "importModal.types.csv.label",
                    MenuCommand::ExportDataset(DatabaseExportFormat::Csv),
                    export_disabled,
                ),
                item(
                    "menubar.parquet",
                    MenuCommand::ExportDataset(DatabaseExportFormat::Parquet),
                    export_disabled,
                ),
            ],
        ),
    ])
}

fn window_menu(context: &MenuContext) -> Menu {
    let tabs_disabled = context.busy || !context.tabs;
    Menu::new(translate("menubar.window")).items([
        item(
            "menubar.splitEditorRight",
            MenuCommand::SplitEditor(Placement::Right),
            context.busy || !context.can_split,
        ),
        item(
            "menubar.splitEditorDown",
            MenuCommand::SplitEditor(Placement::Bottom),
            context.busy || !context.can_split,
        ),
        MenuItem::separator(),
        item("menubar.closeTab", CloseTab, tabs_disabled),
        item(
            "native.workbench.closeOtherTabs",
            CloseOtherTabs,
            tabs_disabled,
        ),
        item("native.workbench.closeAllTabs", CloseAllTabs, tabs_disabled),
    ])
}

fn help_menu(disabled: bool) -> Menu {
    let mut items = vec![
        item(
            "menubar.documentation",
            MenuCommand::Help(HelpPage::Documentation),
            disabled,
        ),
        item(
            "menubar.keyboardShortcuts",
            MenuCommand::ShowKeybindings,
            disabled,
        ),
        item(
            "menubar.releaseNotes",
            MenuCommand::Help(HelpPage::ReleaseNotes),
            disabled,
        ),
        item(
            "menubar.reportIssue",
            MenuCommand::Help(HelpPage::ReportIssue),
            disabled,
        ),
    ];
    if !cfg!(target_os = "macos") {
        items.extend([
            MenuItem::separator(),
            item("menubar.about", MenuCommand::About, disabled),
        ]);
    }
    Menu::new(translate("menubar.help")).items(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A missing locale key is shown as the key itself.
    fn untranslated(label: &str) -> bool {
        label.contains('.') && !label.contains(' ')
    }

    fn check(menu: &Menu) {
        assert!(!untranslated(&menu.name), "{}", menu.name);
        for item in &menu.items {
            match item {
                MenuItem::Submenu(menu) => check(menu),
                MenuItem::Action { name, action, .. } => {
                    assert!(!untranslated(name), "missing label {name}");
                    assert!(
                        !action.partial_eq(&gpui_kit::NoAction),
                        "placeholder item {name}"
                    );
                }
                MenuItem::Separator | MenuItem::SystemMenu(_) => {}
            }
        }
    }

    #[test]
    fn every_item_is_a_labelled_command() {
        for menu in application_menus(&MenuContext::default()) {
            check(&menu);
        }
        for command in super::super::shortcut_commands() {
            assert!(!untranslated(&command.label()), "{}", command.id());
        }
    }
}
