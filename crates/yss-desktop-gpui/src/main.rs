#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use yss_ui::{appearance, text};
use yss_workbench::{Startup, WindowState};

use anyhow::{Result, bail};
use gpui_kit::component::{Root, TitleBar};
use gpui_kit::{App, AppContext, Bounds, WindowBounds, WindowDecorations, WindowOptions, px, size};

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
    let window_state = WindowState::load(executor.clone());
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            yss_ui::init(cx);
            yss_workbench::init(cx);
            let bounds = Bounds::centered(None, window_state.size(), cx);
            cx.open_window(
                WindowOptions {
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some("YssBI".into()),
                        ..TitleBar::title_bar_options()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: Some(yss_ui::APPLICATION_ID.into()),
                    window_min_size: Some(size(px(900.), px(620.))),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..TitleBar::window_options()
                },
                move |window, cx| {
                    appearance::install(window, cx);
                    window.set_window_title("YssBI");
                    let window_state = window_state.attach(window, cx);
                    let startup = cx.new(|cx| {
                        Startup::new(
                            executor,
                            initial_project,
                            initial_resource,
                            window_state,
                            window,
                            cx,
                        )
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
