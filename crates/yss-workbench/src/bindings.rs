use crate::{assistant, canvas, charts, databases, documents, minds, settings, workbench};
use gpui_kit::base::actions::{
    SelectDown, SelectFirst, SelectLast, SelectNextColumn, SelectPageDown, SelectPageUp,
    SelectPrevColumn, SelectUp,
};
use gpui_kit::{Action, App, KeyBinding};
use yss_ui::keymap;

const DATA_TABLE: &str = "DatabaseEditor > DataTable";

/// Binds `keys` with both Ctrl and Cmd. Menus show an action's last binding, so the
/// platform's primary modifier is bound second.
pub(crate) fn primary(keys: &str, action: impl Action + Clone, context: &str) -> [KeyBinding; 2] {
    let [secondary, primary] = if cfg!(target_os = "macos") {
        ["ctrl", "cmd"]
    } else {
        ["cmd", "ctrl"]
    };
    [
        KeyBinding::new(
            &format!("{secondary}-{keys}"),
            action.clone(),
            Some(context),
        ),
        KeyBinding::new(&format!("{primary}-{keys}"), action, Some(context)),
    ]
}

pub(crate) fn init(cx: &mut App) {
    let bindings = [
        vec![KeyBinding::new(
            "escape",
            assistant::CancelResponse,
            Some("AssistantConversation"),
        )],
        primary("o", workbench::OpenProjectDirectory, "Workbench").into(),
        primary("alt-o", workbench::OpenRecentProject, "Workbench").into(),
        primary("s", settings::SaveSettings, "Settings").into(),
        vec![KeyBinding::new(
            "alt-left",
            settings::BackInSettings,
            Some("SettingsBreadcrumb"),
        )],
        primary(",", workbench::ShowSettings, "Settings").into(),
        primary("shift-s", workbench::SaveAllGraphs, "Settings").into(),
        primary("s", charts::SaveChart, "ChartEditor").into(),
        [
            KeyBinding::new("shift-up", SelectUp, Some(DATA_TABLE)),
            KeyBinding::new("shift-down", SelectDown, Some(DATA_TABLE)),
            KeyBinding::new("shift-left", SelectPrevColumn, Some(DATA_TABLE)),
            KeyBinding::new("shift-right", SelectNextColumn, Some(DATA_TABLE)),
            KeyBinding::new("shift-home", SelectFirst, Some(DATA_TABLE)),
            KeyBinding::new("shift-end", SelectLast, Some(DATA_TABLE)),
            KeyBinding::new("shift-pageup", SelectPageUp, Some(DATA_TABLE)),
            KeyBinding::new("shift-pagedown", SelectPageDown, Some(DATA_TABLE)),
        ]
        .into(),
        primary("c", databases::CopyDatabaseSelection, "DatabaseEditor").into(),
        primary("a", databases::SelectDatabasePage, "DatabaseEditor").into(),
        vec![KeyBinding::new(
            "escape",
            databases::ClearDatabaseSelection,
            Some("DatabaseEditor"),
        )],
        primary("s", minds::SaveMind, "MindCanvas").into(),
        primary("a", minds::SelectTopics, "MindCanvas").into(),
        vec![
            KeyBinding::new("delete", minds::DeleteTopics, Some("MindCanvas")),
            KeyBinding::new("backspace", minds::DeleteTopics, Some("MindCanvas")),
            KeyBinding::new("escape", minds::CancelMindGesture, Some("MindCanvas")),
            KeyBinding::new("home", minds::FitMind, Some("MindCanvas")),
            KeyBinding::new("f", minds::FitTopics, Some("MindCanvas")),
        ],
        primary("s", documents::SaveDocument, "DocumentEditor").into(),
        primary(
            "shift-v",
            documents::ToggleDocumentPreview,
            "DocumentEditor",
        )
        .into(),
        primary("s", canvas::SaveGraph, "GraphCanvas").into(),
        primary("z", canvas::UndoGraph, "GraphCanvas && !Input").into(),
        primary("shift-z", canvas::RedoGraph, "GraphCanvas && !Input").into(),
        primary("y", canvas::RedoGraph, "GraphCanvas && !Input").into(),
        primary("a", canvas::SelectAll, "GraphCanvas && !Input").into(),
        primary("x", gpui_kit::component::input::Cut, "GraphCanvas").into(),
        primary("c", gpui_kit::component::input::Copy, "GraphCanvas").into(),
        primary("v", gpui_kit::component::input::Paste, "GraphCanvas").into(),
        primary("d", canvas::DuplicateSelection, "GraphCanvas && !Input").into(),
        primary("space", canvas::OpenNodePalette, "GraphCanvas && !Input").into(),
        vec![
            KeyBinding::new(
                "delete",
                canvas::DeleteSelection,
                Some("GraphCanvas && !Input"),
            ),
            KeyBinding::new("escape", canvas::CancelGesture, Some("GraphCanvas")),
            KeyBinding::new("home", canvas::FrameGraph, Some("GraphCanvas")),
            KeyBinding::new("f", canvas::FrameSelection, Some("GraphCanvas && !Input")),
            KeyBinding::new("f5", canvas::RunWholeGraph, Some("GraphCanvas")),
            KeyBinding::new("shift-f5", canvas::CancelRun, Some("GraphCanvas")),
            KeyBinding::new("f5", canvas::RunWholeGraph, Some("Workbench")),
            KeyBinding::new("shift-f5", canvas::CancelRun, Some("Workbench")),
        ],
    ];
    keymap::init(cx, bindings.into_iter().flatten(), |cx| {
        workbench::bind_menu_keys(cx);
        workbench::shortcut_commands()
    });
}
