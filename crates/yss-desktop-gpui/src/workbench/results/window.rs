//! Native result windows reuse ResultPanel, with separate controls and Application leases.
use super::super::Workbench;
use crate::{results::ResultPanel, window_chrome};
use gpui_kit::component::{ActiveTheme, Root, TitleBar, dock::Panel};
use gpui_kit::{
    App, AppContext, Bounds, Context, Entity, Focusable, IntoElement, Render, Subscription,
    WeakEntity, Window, WindowBounds, WindowDecorations, WindowHandle, WindowOptions, div,
    prelude::*, px, size,
};

pub(in crate::workbench) struct ResultWindowHandle {
    window: WindowHandle<Root>,
    panel: WeakEntity<ResultPanel>,
}

impl ResultWindowHandle {
    fn close(&self, cx: &mut App) {
        if let Some(panel) = self.panel.upgrade() {
            panel.update(cx, |panel, cx| panel.close(cx));
        }
        let handle = self.window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        });
    }
}

impl Workbench {
    pub(super) fn open_result_window(
        &mut self,
        source: &Entity<ResultPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(handoff) = source.read(cx).retain_for_window() else {
            return;
        };
        let reference = handoff.reference();
        let owner = cx.weak_entity();
        let lifecycle = self.lifecycle;
        let services = self.services.clone();
        let bounds = Bounds::centered(
            window.display(cx).map(|display| display.id()),
            size(px(1000.), px(720.)),
            cx,
        );
        // Window creation/activation can render synchronously; return the source to GPUI first.
        cx.defer(move |cx| {
            let Some(workbench) = owner.upgrade() else {
                return;
            };
            if workbench.read(cx).lifecycle != lifecycle || workbench.read(cx).is_closing(cx) {
                return;
            }
            workbench.update(cx, |view, cx| view.close_stale_result_windows(cx));
            if !services
                .application
                .application
                .capture_session()
                .is_ok_and(|session| {
                    session.execution_session_id() == reference.execution_session_id
                })
            {
                workbench.update(cx, |view, cx| {
                    view.error = Some(crate::text::translate("native.results.unavailable"));
                    cx.notify();
                });
                return;
            }
            let existing = workbench.read(cx).result_windows.iter().find_map(|entry| {
                let panel = entry.panel.upgrade()?;
                (panel.read(cx).available() && panel.read(cx).reference() == reference)
                    .then_some((entry.window, panel))
            });
            if let Some((handle, panel)) = existing
                && handle
                    .update(cx, |_, window, cx| {
                        window.activate_window();
                        window.focus(&panel.read(cx).focus_handle(cx), cx);
                    })
                    .is_ok()
            {
                return;
            }
            let panel = cx.new(|cx| ResultPanel::new(services, reference, cx));
            let weak = panel.downgrade();
            let opened = cx.open_window(
                WindowOptions {
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some(panel.read(cx).window_title().into()),
                        ..TitleBar::title_bar_options()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: Some("com.zjy.yssbi".into()),
                    window_min_size: Some(size(px(480.), px(360.))),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..TitleBar::window_options()
                },
                move |window, cx| {
                    crate::appearance::install(window, cx);
                    panel.update(cx, |panel, cx| {
                        panel.load_retained(Some(handoff), window, cx);
                    });
                    window.focus(&panel.read(cx).focus_handle(cx), cx);
                    let view = cx.new(|cx| ResultWindow::new(panel, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            workbench.update(cx, |view, cx| match opened {
                Ok(window) => view.result_windows.push(ResultWindowHandle {
                    window,
                    panel: weak,
                }),
                Err(_) => {
                    view.error = Some(crate::text::translate("native.results.windowFailed"));
                    cx.notify();
                }
            });
        });
    }

    pub(in crate::workbench) fn close_result_windows(&mut self, cx: &mut App) {
        for entry in self.result_windows.drain(..) {
            entry.close(cx);
        }
    }

    pub(in crate::workbench) fn close_stale_result_windows(&mut self, cx: &mut App) {
        let current = self
            .services
            .application
            .application
            .capture_session()
            .ok()
            .map(|session| session.execution_session_id());
        self.result_windows.retain(|entry| {
            let Some(panel) = entry.panel.upgrade() else {
                return false;
            };
            if entry.window.read(cx).is_err() || !panel.read(cx).available() {
                return false;
            }
            if current
                .is_some_and(|current| panel.read(cx).reference().execution_session_id != current)
            {
                entry.close(cx);
                return false;
            }
            true
        });
    }
}

struct ResultWindow {
    panel: Entity<ResultPanel>,
    _subscription: Subscription,
}

impl ResultWindow {
    fn new(panel: Entity<ResultPanel>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&panel, |_, _, cx| cx.notify());
        cx.on_release(|view, cx| {
            view.panel.update(cx, |panel, cx| panel.close(cx));
        })
        .detach();
        Self {
            panel,
            _subscription: subscription,
        }
    }
}

impl Render for ResultWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(&self.panel.read(cx).window_title());
        let title = self
            .panel
            .update(cx, |panel, cx| panel.title(window, cx).into_any_element());
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(window_chrome::title_bar(
                |_, window, _| window.remove_window(),
                title,
                window,
                cx,
            ))
            .child(div().flex_1().min_h_0().min_w_0().child(self.panel.clone()))
    }
}
