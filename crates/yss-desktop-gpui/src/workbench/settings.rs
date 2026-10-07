//! The settings window shares the workbench's existing draft and model services.
use super::{SaveAllGraphs, ShowSettings, Workbench, menus::MenuCommand};
use crate::{canvas::SaveGraph, settings::SettingsPanel, window_chrome};
use gpui::{
    AnyWindowHandle, AppContext, Bounds, Context, Entity, Focusable, IntoElement, Render,
    Subscription, WeakEntity, Window, WindowBounds, WindowDecorations, WindowOptions, div,
    prelude::*, px, size,
};
use gpui_component::{ActiveTheme, Icon, IconName, Root, TitleBar};

impl Workbench {
    pub(super) fn show_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window
            && handle
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    window.focus(&self.settings.read(cx).focus_handle(cx), cx);
                })
                .is_ok()
        {
            return;
        }
        if self.is_closing(cx) {
            return;
        }
        let panel = self.settings.clone();
        let owner = cx.weak_entity();
        let owner_window = window.window_handle();
        let bounds = Bounds::centered(
            window.display(cx).map(|display| display.id()),
            size(px(1000.), px(720.)),
            cx,
        );
        match cx.open_window(
            WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("设置 — YssBI".into()),
                    ..TitleBar::title_bar_options()
                }),
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("com.zjy.yssbi".into()),
                window_min_size: Some(size(px(760.), px(560.))),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..TitleBar::window_options()
            },
            move |window, cx| {
                let close_panel = panel.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    close_panel
                        .upgrade()
                        .is_none_or(|panel| !panel.read(cx).busy())
                });
                let view = cx.new(|cx| SettingsWindow::new(panel, owner, owner_window, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(handle) => self.settings_window = Some(handle),
            Err(_) => self.error = Some("设置窗口未能打开，请重试。".into()),
        }
        cx.notify();
    }
}

struct SettingsWindow {
    panel: Entity<SettingsPanel>,
    owner: WeakEntity<Workbench>,
    owner_window: AnyWindowHandle,
    _subscription: Subscription,
}

impl SettingsWindow {
    fn new(
        panel: Entity<SettingsPanel>,
        owner: WeakEntity<Workbench>,
        owner_window: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.observe(&panel, |_, _, cx| cx.notify());
        cx.defer_in(window, |view, window, cx| {
            window.focus(&view.panel.read(cx).focus_handle(cx), cx);
            view.panel.update(cx, |panel, cx| panel.reload(window, cx));
        });
        Self {
            panel,
            owner,
            owner_window,
            _subscription: subscription,
        }
    }

    fn in_workbench(
        &self,
        cx: &mut Context<Self>,
        command: impl FnOnce(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
    ) {
        let owner = self.owner.clone();
        let owner_window = self.owner_window;
        // Return the current window before forwarding actions to another window.
        cx.defer(move |cx| {
            let _ = owner_window.update(cx, |_, window, cx| {
                if let Some(owner) = owner.upgrade() {
                    window.activate_window();
                    owner.update(cx, |view, cx| command(view, window, cx));
                }
            });
        });
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if self.panel.read(cx).dirty() {
            "设置 • — YssBI"
        } else {
            "设置 — YssBI"
        };
        window.set_window_title(title);
        div()
            .key_context("Settings")
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|view, _: &SaveGraph, window, cx| {
                view.panel.update(cx, |panel, cx| panel.save(window, cx));
            }))
            .on_action(cx.listener(|view, _: &ShowSettings, window, cx| {
                window.focus(&view.panel.read(cx).focus_handle(cx), cx);
            }))
            .on_action(cx.listener(|view, _: &SaveAllGraphs, _, cx| {
                view.in_workbench(cx, |view, window, cx| {
                    view.save_all(super::lifecycle::AfterSave::Stay, window, cx);
                });
            }))
            .on_action(cx.listener(|view, command: &MenuCommand, _, cx| {
                let command = command.clone();
                view.in_workbench(cx, move |view, window, cx| {
                    view.dispatch_menu(&command, window, cx);
                });
            }))
            .child(window_chrome::title_bar(
                cx.listener(|view, _, window, cx| {
                    if !view.panel.read(cx).busy() {
                        window.remove_window();
                    }
                }),
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .child(Icon::new(IconName::Settings).size_4())
                    .child(title),
                window,
                cx,
            ))
            .child(div().flex_1().min_h_0().child(self.panel.clone()))
    }
}
