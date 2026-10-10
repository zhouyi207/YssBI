//! A singleton log window with its own view subscription to the existing LogRuntime.
use super::{super::Workbench, LogsPanel};
use crate::window_chrome;
use gpui::{
    AppContext, Bounds, Context, Entity, Focusable, IntoElement, Render, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::{ActiveTheme, Root, TitleBar};

impl Workbench {
    pub(in crate::workbench) fn show_logs_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.logs_window
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            return;
        }
        let services = self.services.clone();
        let bounds = Bounds::centered(
            window.display(cx).map(|display| display.id()),
            size(px(1000.), px(640.)),
            cx,
        );
        match cx.open_window(
            WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(crate::text::t("native.workbench.logsWindowTitle").into()),
                    ..TitleBar::title_bar_options()
                }),
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("com.zjy.yssbi".into()),
                window_min_size: Some(size(px(640.), px(400.))),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..TitleBar::window_options()
            },
            move |window, cx| {
                let panel = cx.new(|cx| LogsPanel::new(services, window, cx));
                window.focus(&panel.read(cx).focus_handle(cx), cx);
                let view = cx.new(|_| LogsWindow { panel });
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(handle) => self.logs_window = Some(handle),
            Err(_) => self.error = Some(crate::text::t("native.workbench.logsWindowFailed").into()),
        }
        cx.notify();
    }
}

struct LogsWindow {
    panel: Entity<LogsPanel>,
}

impl Render for LogsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(window_chrome::title_bar(
                |_, window, _| window.remove_window(),
                div()
                    .text_sm()
                    .child(crate::text::t("native.workbench.logsWindowTitle")),
                window,
                cx,
            ))
            .child(div().flex_1().min_h_0().child(self.panel.clone()))
    }
}
