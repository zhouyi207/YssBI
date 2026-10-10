//! Mirrors the ordering, grouping and locale keys of the React menu contribution registry.
use super::{MenuCommand, MenuContext, WorkbenchPanel, editing::EditCommand, help::HelpPage};
use crate::{
    canvas::SaveGraph,
    text::translate,
    workbench::{OpenProjectDirectory, ShowSettings},
};
use gpui::{Action, Menu, MenuItem};
use gpui_component::Placement;
use yss_graph_document::GraphResourceKind;

fn item(key: &str, action: impl Action, disabled: bool) -> MenuItem {
    MenuItem::action(translate(key), action).disabled(disabled)
}

pub(super) fn application_menus(context: MenuContext) -> Vec<Menu> {
    let mut menus = vec![];
    if cfg!(target_os = "macos") {
        menus.push(
            Menu::new("YssBI").items([
                item("menubar.about", MenuCommand::About, context.busy),
                MenuItem::separator(),
                item("menubar.settings", ShowSettings, context.busy),
                MenuItem::separator(),
                MenuItem::action(crate::text::t("native.workbench.quit"), MenuCommand::Exit)
                    .disabled(context.busy),
            ]),
        );
    }
    menus.extend([
        file_menu(context),
        edit_menu(context),
        Menu::new(translate("menubar.data")).items([
            item(
                "menubar.importData",
                MenuCommand::ImportData,
                context.busy || !context.project,
            ),
            MenuItem::separator(),
            item("menubar.schemaViewer", gpui::NoAction, true),
        ]),
        Menu::new(translate("menubar.view")).items([
            item(
                "panel.primarySideBar",
                MenuCommand::ToggleSidebar,
                context.busy || !context.project,
            )
            .checked(context.sidebar_open),
            item(
                "panel.assistant",
                MenuCommand::ShowPanel(WorkbenchPanel::Assistant),
                context.busy,
            )
            .checked(context.assistant_open),
            MenuItem::separator(),
            item(
                "menubar.resetLayout",
                MenuCommand::ResetLayout,
                context.busy || !context.project,
            ),
        ]),
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
            item(
                "menubar.openLogsInNewWindow",
                MenuCommand::OpenLogsWindow,
                context.busy,
            ),
        ]),
        Menu::new(translate("menubar.tools")).items([
            item("menubar.debugger", gpui::NoAction, true),
            item("menubar.profiler", gpui::NoAction, true),
            MenuItem::separator(),
            item("menubar.settings", ShowSettings, context.busy),
        ]),
        help_menu(context.busy),
    ]);
    menus
}

fn file_menu(context: MenuContext) -> Menu {
    let disabled = context.busy || !context.project;
    Menu::new(translate("menubar.file")).items([
        item(
            "menubar.newEventGraph",
            MenuCommand::NewGraph(GraphResourceKind::EventGraph),
            disabled,
        ),
        item(
            "menubar.newFunctionGraph",
            MenuCommand::NewGraph(GraphResourceKind::FunctionGraph),
            disabled,
        ),
        item("menubar.newChart", MenuCommand::NewChart, disabled),
        item("documents.newMind", MenuCommand::NewMind, disabled),
        item("documents.newDoc", MenuCommand::NewDocument, disabled),
        MenuItem::separator(),
        item("menubar.openProject", OpenProjectDirectory, context.busy),
        item("menubar.closeProject", MenuCommand::CloseProject, disabled),
        MenuItem::separator(),
        item(
            "common.save",
            SaveGraph,
            disabled || context.editor.is_none(),
        ),
        item(
            "menubar.saveProjectAs",
            MenuCommand::SaveProjectAs,
            disabled,
        ),
    ])
}

fn edit_menu(context: MenuContext) -> Menu {
    let edits = [
        ("common.undo", EditCommand::Undo),
        ("common.redo", EditCommand::Redo),
        ("menubar.cut", EditCommand::Cut),
        ("menubar.copy", EditCommand::Copy),
        ("menubar.paste", EditCommand::Paste),
        ("common.delete", EditCommand::Delete),
    ];
    let mut items = vec![];
    for (ix, (key, command)) in edits.into_iter().enumerate() {
        if ix == 2 || ix == 5 {
            items.push(MenuItem::separator());
        }
        items.push(item(
            key,
            MenuCommand::Edit(command),
            context.busy || !context.edits[ix],
        ));
    }
    Menu::new(translate("menubar.edit")).items(items)
}

fn help_menu(disabled: bool) -> Menu {
    Menu::new(translate("menubar.help")).items([
        item(
            "menubar.architecture",
            MenuCommand::Help(HelpPage::Architecture),
            disabled,
        ),
        item(
            "menubar.documentation",
            MenuCommand::Help(HelpPage::Documentation),
            disabled,
        ),
        MenuItem::separator(),
        item(
            "menubar.releaseNotes",
            MenuCommand::Help(HelpPage::ReleaseNotes),
            disabled,
        ),
        item(
            "menubar.githubRepository",
            MenuCommand::Help(HelpPage::Repository),
            disabled,
        ),
        item(
            "menubar.reportIssue",
            MenuCommand::Help(HelpPage::ReportIssue),
            disabled,
        ),
        MenuItem::separator(),
        item("menubar.about", MenuCommand::About, disabled),
    ])
}
