#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appearance;
mod assistant;
mod canvas;
mod catalog_rows;
mod charts;
mod constant_values;
mod databases;
mod documents;
mod file_commands;
mod imports;
mod markdown;
mod minds;
mod modal_window;
mod plots;
mod plugins;
mod project;
mod projects;
mod results;
mod services;
mod settings;
mod startup;
mod text;
mod window_chrome;
mod workbench;

use anyhow::{Result, bail};
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::{
    App, AppContext, Bounds, KeyBinding, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use startup::Startup;

fn main() -> Result<()> {
    text::set_locale(text::DEFAULT_LANGUAGE);
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() > 2 {
        bail!(crate::text::t("native.main.usage"));
    }
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("{}", crate::text::t("native.main.usage"));
        return Ok(());
    }
    // The runtime outlives the GPUI event loop; no blocking work runs in pointer handlers.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let executor = runtime.handle().clone();
    let initial_project = args.first().map(std::path::PathBuf::from);
    let initial_resource = args
        .get(1)
        .map(|value| value.to_string_lossy().into_owned());
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            markdown::init();
            window_chrome::init(cx);
            modal_window::init(cx);
            cx.bind_keys([
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
                KeyBinding::new("ctrl-z", canvas::UndoGraph, Some("GraphCanvas")),
                KeyBinding::new("ctrl-shift-z", canvas::RedoGraph, Some("GraphCanvas")),
                KeyBinding::new("ctrl-y", canvas::RedoGraph, Some("GraphCanvas")),
                KeyBinding::new("ctrl-a", canvas::SelectAll, Some("GraphCanvas")),
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
                KeyBinding::new("ctrl-d", canvas::DuplicateSelection, Some("GraphCanvas")),
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
                KeyBinding::new("cmd-d", canvas::DuplicateSelection, Some("GraphCanvas")),
                KeyBinding::new("delete", canvas::DeleteSelection, Some("GraphCanvas")),
                KeyBinding::new("escape", canvas::CancelGesture, Some("GraphCanvas")),
                KeyBinding::new("home", canvas::FrameGraph, Some("GraphCanvas")),
                KeyBinding::new("f", canvas::FrameSelection, Some("GraphCanvas && !Input")),
                KeyBinding::new("f5", canvas::RunWholeGraph, Some("GraphCanvas")),
                KeyBinding::new("shift-f5", canvas::CancelRun, Some("GraphCanvas")),
                KeyBinding::new("cmd-s", canvas::SaveGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-z", canvas::UndoGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-shift-z", canvas::RedoGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-a", canvas::SelectAll, Some("GraphCanvas")),
                KeyBinding::new("f5", canvas::RunWholeGraph, Some("Workbench")),
                KeyBinding::new("shift-f5", canvas::CancelRun, Some("Workbench")),
            ]);
            workbench::bind_menu_keys(cx);
            let bounds = Bounds::centered(None, size(px(1480.), px(940.)), cx);
            cx.open_window(
                WindowOptions {
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some("YssBI".into()),
                        ..TitleBar::title_bar_options()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: Some("com.zjy.yssbi".into()),
                    window_min_size: Some(size(px(900.), px(620.))),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..TitleBar::window_options()
                },
                move |window, cx| {
                    appearance::install(window, cx);
                    window.set_window_title("YssBI");
                    let startup = cx.new(|cx| {
                        Startup::new(executor, initial_project, initial_resource, window, cx)
                    });
                    let weak = startup.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        let Some(view) = weak.upgrade() else {
                            return true;
                        };
                        view.update(cx, |view, cx| view.close_requested(window, cx))
                    });
                    cx.new(|cx| Root::new(startup, window, cx))
                },
            )
            .unwrap_or_else(|_| panic!("{}", crate::text::t("native.main.windowFailed")));
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
    Ok(())
}
