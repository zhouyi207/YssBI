#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod appearance;
mod assets;
mod assistant;
mod canvas;
mod charts;
mod databases;
mod documents;
mod file_commands;
mod imports;
mod minds;
mod plots;
mod plugins;
mod project;
mod projects;
mod results;
mod services;
mod settings;
mod text;
mod window_chrome;
mod workbench;

use anyhow::{Result, bail};
use gpui::{
    App, AppContext, Bounds, KeyBinding, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use gpui_component::{Root, TitleBar};
use project::DesktopProject;
use services::NativeServices;
use workbench::Workbench;

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() > 2 {
        bail!("用法: cargo run -- [项目目录] [资源相对路径或数据库 ID]");
    }
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("用法: cargo run -- [项目目录] [资源相对路径或数据库 ID]");
        return Ok(());
    }
    // The runtime outlives the GPUI event loop; no blocking work runs in pointer handlers.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let services = runtime.block_on(NativeServices::initialize(runtime.handle().clone()))?;
    let project = args
        .first()
        .map(|root| {
            let activation = services
                .application
                .application
                .load_project_for_application(&root.to_string_lossy())?;
            let snapshot = services.application.application.query_project_index(
                activation.project_instance_id.clone(),
                "zh-CN",
                true,
            )?;
            if runtime
                .block_on(
                    services
                        .application
                        .projects
                        .register_project(&snapshot.index.project_name, &activation.path),
                )
                .is_err()
            {
                tracing::warn!(
                    code = "native_project_registration_failed",
                    "Project opened without a registry update"
                );
            }
            services.watch_project(&activation.project_instance_id)?;
            Ok::<_, anyhow::Error>(DesktopProject::new(
                activation.project_instance_id,
                snapshot,
            ))
        })
        .transpose()?;
    let initial_resource = args
        .get(1)
        .map(|value| value.to_string_lossy().into_owned());
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            window_chrome::init(cx);
            cx.bind_keys([
                KeyBinding::new(
                    "ctrl-enter",
                    assistant::SendMessage,
                    Some("AssistantConversation"),
                ),
                KeyBinding::new(
                    "cmd-enter",
                    assistant::SendMessage,
                    Some("AssistantConversation"),
                ),
                KeyBinding::new(
                    "escape",
                    assistant::CancelResponse,
                    Some("AssistantConversation"),
                ),
                KeyBinding::new("ctrl-,", workbench::ShowSettings, Some("Workbench")),
                KeyBinding::new("cmd-,", workbench::ShowSettings, Some("Workbench")),
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
                    gpui_base::actions::SelectUp,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-down",
                    gpui_base::actions::SelectDown,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-left",
                    gpui_base::actions::SelectPrevColumn,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-right",
                    gpui_base::actions::SelectNextColumn,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-home",
                    gpui_base::actions::SelectFirst,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-end",
                    gpui_base::actions::SelectLast,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-pageup",
                    gpui_base::actions::SelectPageUp,
                    Some("DatabaseEditor > DataTable"),
                ),
                KeyBinding::new(
                    "shift-pagedown",
                    gpui_base::actions::SelectPageDown,
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
                KeyBinding::new("delete", canvas::DeleteSelection, Some("GraphCanvas")),
                KeyBinding::new("escape", canvas::CancelGesture, Some("GraphCanvas")),
                KeyBinding::new("home", canvas::FrameGraph, Some("GraphCanvas")),
                KeyBinding::new("f5", canvas::RunWholeGraph, Some("GraphCanvas")),
                KeyBinding::new("shift-f5", canvas::CancelRun, Some("GraphCanvas")),
                KeyBinding::new("cmd-s", canvas::SaveGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-z", canvas::UndoGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-shift-z", canvas::RedoGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-a", canvas::SelectAll, Some("GraphCanvas")),
                KeyBinding::new("ctrl-s", canvas::SaveGraph, Some("Workbench")),
                KeyBinding::new("cmd-s", canvas::SaveGraph, Some("Workbench")),
                KeyBinding::new("ctrl-shift-s", workbench::SaveAllGraphs, Some("Workbench")),
                KeyBinding::new("cmd-shift-s", workbench::SaveAllGraphs, Some("Workbench")),
                KeyBinding::new("f5", canvas::RunWholeGraph, Some("Workbench")),
                KeyBinding::new("shift-f5", canvas::CancelRun, Some("Workbench")),
            ]);
            let bounds = Bounds::centered(None, size(px(1480.), px(940.)), cx);
            cx.open_window(
                WindowOptions {
                    titlebar: Some(gpui::TitlebarOptions {
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
                    let workbench = cx
                        .new(|cx| Workbench::new(services, project, initial_resource, window, cx));
                    let weak = workbench.downgrade();
                    window.on_window_should_close(cx, move |window, cx| {
                        let Some(view) = weak.upgrade() else {
                            return true;
                        };
                        view.update(cx, |view, cx| view.close_requested(window, cx))
                    });
                    cx.new(|cx| Root::new(workbench, window, cx))
                },
            )
            .expect("无法创建 GPUI 窗口");
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
