use crate::{assistant, canvas, charts, databases, documents, minds, settings, workbench};
use gpui_kit::{App, KeyBinding};
use yss_ui::keymap;

pub(crate) fn init(cx: &mut App) {
    keymap::init(
        cx,
        [
            KeyBinding::new(
                "escape",
                assistant::CancelResponse,
                Some("AssistantConversation"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "ctrl-o"
                } else {
                    "cmd-o"
                },
                workbench::OpenProjectDirectory,
                Some("Workbench"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-o"
                } else {
                    "ctrl-o"
                },
                workbench::OpenProjectDirectory,
                Some("Workbench"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "ctrl-alt-o"
                } else {
                    "cmd-alt-o"
                },
                workbench::OpenRecentProject,
                Some("Workbench"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-alt-o"
                } else {
                    "ctrl-alt-o"
                },
                workbench::OpenRecentProject,
                Some("Workbench"),
            ),
            KeyBinding::new("ctrl-s", settings::SaveSettings, Some("Settings")),
            KeyBinding::new("cmd-s", settings::SaveSettings, Some("Settings")),
            KeyBinding::new(
                "alt-left",
                settings::BackInSettings,
                Some("SettingsBreadcrumb"),
            ),
            KeyBinding::new("ctrl-,", workbench::ShowSettings, Some("Settings")),
            KeyBinding::new("cmd-,", workbench::ShowSettings, Some("Settings")),
            KeyBinding::new("ctrl-shift-s", workbench::SaveAllGraphs, Some("Settings")),
            KeyBinding::new("cmd-shift-s", workbench::SaveAllGraphs, Some("Settings")),
            KeyBinding::new("ctrl-s", charts::SaveChart, Some("ChartEditor")),
            KeyBinding::new("cmd-s", charts::SaveChart, Some("ChartEditor")),
            KeyBinding::new(
                "shift-up",
                gpui_kit::base::actions::SelectUp,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-down",
                gpui_kit::base::actions::SelectDown,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-left",
                gpui_kit::base::actions::SelectPrevColumn,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-right",
                gpui_kit::base::actions::SelectNextColumn,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-home",
                gpui_kit::base::actions::SelectFirst,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-end",
                gpui_kit::base::actions::SelectLast,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-pageup",
                gpui_kit::base::actions::SelectPageUp,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "shift-pagedown",
                gpui_kit::base::actions::SelectPageDown,
                Some("DatabaseEditor > DataTable"),
            ),
            KeyBinding::new(
                "ctrl-c",
                databases::CopyDatabaseSelection,
                Some("DatabaseEditor"),
            ),
            KeyBinding::new(
                "cmd-c",
                databases::CopyDatabaseSelection,
                Some("DatabaseEditor"),
            ),
            KeyBinding::new(
                "ctrl-a",
                databases::SelectDatabasePage,
                Some("DatabaseEditor"),
            ),
            KeyBinding::new(
                "cmd-a",
                databases::SelectDatabasePage,
                Some("DatabaseEditor"),
            ),
            KeyBinding::new(
                "escape",
                databases::ClearDatabaseSelection,
                Some("DatabaseEditor"),
            ),
            KeyBinding::new("ctrl-s", minds::SaveMind, Some("MindCanvas")),
            KeyBinding::new("cmd-s", minds::SaveMind, Some("MindCanvas")),
            KeyBinding::new("ctrl-a", minds::SelectTopics, Some("MindCanvas")),
            KeyBinding::new("cmd-a", minds::SelectTopics, Some("MindCanvas")),
            KeyBinding::new("delete", minds::DeleteTopics, Some("MindCanvas")),
            KeyBinding::new("backspace", minds::DeleteTopics, Some("MindCanvas")),
            KeyBinding::new("escape", minds::CancelMindGesture, Some("MindCanvas")),
            KeyBinding::new("home", minds::FitMind, Some("MindCanvas")),
            KeyBinding::new("f", minds::FitTopics, Some("MindCanvas")),
            KeyBinding::new("ctrl-s", documents::SaveDocument, Some("DocumentEditor")),
            KeyBinding::new("cmd-s", documents::SaveDocument, Some("DocumentEditor")),
            KeyBinding::new(
                "ctrl-shift-v",
                documents::ToggleDocumentPreview,
                Some("DocumentEditor"),
            ),
            KeyBinding::new(
                "cmd-shift-v",
                documents::ToggleDocumentPreview,
                Some("DocumentEditor"),
            ),
            KeyBinding::new("ctrl-s", canvas::SaveGraph, Some("GraphCanvas")),
            KeyBinding::new("ctrl-z", canvas::UndoGraph, Some("GraphCanvas && !Input")),
            KeyBinding::new(
                "ctrl-shift-z",
                canvas::RedoGraph,
                Some("GraphCanvas && !Input"),
            ),
            KeyBinding::new("ctrl-y", canvas::RedoGraph, Some("GraphCanvas && !Input")),
            KeyBinding::new("ctrl-a", canvas::SelectAll, Some("GraphCanvas && !Input")),
            KeyBinding::new(
                "ctrl-c",
                gpui_kit::component::input::Copy,
                Some("GraphCanvas"),
            ),
            KeyBinding::new(
                "ctrl-x",
                gpui_kit::component::input::Cut,
                Some("GraphCanvas"),
            ),
            KeyBinding::new(
                "ctrl-v",
                gpui_kit::component::input::Paste,
                Some("GraphCanvas"),
            ),
            KeyBinding::new(
                "ctrl-d",
                canvas::DuplicateSelection,
                Some("GraphCanvas && !Input"),
            ),
            KeyBinding::new(
                "cmd-c",
                gpui_kit::component::input::Copy,
                Some("GraphCanvas"),
            ),
            KeyBinding::new(
                "cmd-x",
                gpui_kit::component::input::Cut,
                Some("GraphCanvas"),
            ),
            KeyBinding::new(
                "cmd-v",
                gpui_kit::component::input::Paste,
                Some("GraphCanvas"),
            ),
            KeyBinding::new(
                "cmd-d",
                canvas::DuplicateSelection,
                Some("GraphCanvas && !Input"),
            ),
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
            KeyBinding::new("cmd-s", canvas::SaveGraph, Some("GraphCanvas")),
            KeyBinding::new("cmd-z", canvas::UndoGraph, Some("GraphCanvas && !Input")),
            KeyBinding::new(
                "cmd-shift-z",
                canvas::RedoGraph,
                Some("GraphCanvas && !Input"),
            ),
            KeyBinding::new("cmd-a", canvas::SelectAll, Some("GraphCanvas && !Input")),
            KeyBinding::new(
                "ctrl-space",
                canvas::OpenNodePalette,
                Some("GraphCanvas && !Input"),
            ),
            KeyBinding::new(
                "cmd-space",
                canvas::OpenNodePalette,
                Some("GraphCanvas && !Input"),
            ),
            KeyBinding::new("f5", canvas::RunWholeGraph, Some("Workbench")),
            KeyBinding::new("shift-f5", canvas::CancelRun, Some("Workbench")),
        ],
        |cx| {
            workbench::bind_menu_keys(cx);
            workbench::shortcut_commands()
        },
    );
}
