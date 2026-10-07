mod appearance;
mod assets;
mod canvas;
mod project;
mod results;
mod services;
mod text;
mod workbench;

use anyhow::{Result, bail};
use gpui::{
    App, AppContext, Bounds, KeyBinding, WindowBounds, WindowDecorations,
    WindowOptions, px, size,
};
use gpui_component::{Root, TitleBar};
use project::DesktopProject;
use services::NativeServices;
use workbench::Workbench;

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() > 2 {
        bail!("用法: pnpm dev:gpui [项目目录] [图相对路径]");
    }
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("用法: pnpm dev:gpui [项目目录] [图相对路径]");
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
            services.watch_project(&activation.project_instance_id)?;
            Ok::<_, anyhow::Error>(DesktopProject::new(
                activation.project_instance_id,
                snapshot,
            ))
        })
        .transpose()?;
    let initial_graph = args
        .get(1)
        .map(|value| value.to_string_lossy().into_owned());
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.bind_keys([
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
                    ..Default::default()
                },
                move |window, cx| {
                    appearance::install(window, cx);
                    window.set_window_title("YssBI");
                    let workbench =
                        cx.new(|cx| Workbench::new(services, project, initial_graph, window, cx));
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
            cx.on_window_closed(|_, cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
    Ok(())
}
