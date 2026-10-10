//! The settings window shares the workbench's existing draft and model services.
use super::{SaveAllGraphs, ShowSettings, Workbench, menus::MenuCommand};
use crate::{
    canvas::SaveGraph,
    settings::{SettingsEvent, SettingsPanel},
    window_chrome,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Root, TitleBar};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, Entity, Focusable, IntoElement, Render,
    Subscription, WeakEntity, Window, WindowBounds, WindowDecorations, WindowOptions, div,
    prelude::*, px, size,
};

impl Workbench {
    pub(super) fn connect_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self.settings.read(cx);
        let mut state = (panel.dirty(), panel.has_pending_operation());
        let mut language = crate::text::locale();
        self.settings_subscription =
            Some(
                cx.observe_in(&self.settings, window, move |view, panel, window, cx| {
                    let panel = panel.read(cx);
                    let next = (panel.dirty(), panel.has_pending_operation());
                    if language != crate::text::locale() {
                        language = crate::text::locale();
                        view.menu_context = None;
                        view.refresh_project(window, cx);
                        view.refresh_graphs(cx);
                        cx.notify();
                    }
                    // Settings content and catalog loading do not change workbench chrome or menus.
                    if next != state {
                        state = next;
                        cx.notify();
                    }
                }),
            );
    }

    pub(super) fn show_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = self.settings.clone();
        let owner = cx.weak_entity();
        let owner_window = window.window_handle();
        let bounds = Bounds::centered(
            window.display(cx).map(|display| display.id()),
            size(px(1000.), px(720.)),
            cx,
        );
        // Return the workbench to GPUI before creating or activating another window.
        cx.defer(move |cx| {
            let Some(workbench) = owner.upgrade() else {
                return;
            };
            // Read the handle here so queued requests also reuse a newly opened window.
            let existing_window = workbench.read(cx).settings_window;
            if let Some(handle) = existing_window
                && handle
                    .update(cx, |_, window, cx| {
                        window.activate_window();
                        window.focus(&panel.read(cx).focus_handle(cx), cx);
                    })
                    .is_ok()
            {
                return;
            }
            if workbench.read(cx).is_closing(cx) {
                return;
            }
            let result = cx.open_window(
                WindowOptions {
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some(crate::text::t("native.workbench.settingsWindowTitle").into()),
                        ..TitleBar::title_bar_options()
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    app_id: Some("com.zjy.yssbi".into()),
                    window_min_size: Some(size(px(600.), px(480.))),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..TitleBar::window_options()
                },
                move |window, cx| {
                    let close_panel = panel.downgrade();
                    window.on_window_should_close(cx, move |_, cx| {
                        close_panel
                            .upgrade()
                            .is_none_or(|panel| !panel.read(cx).has_pending_operation())
                    });
                    let view =
                        cx.new(|cx| SettingsWindow::new(panel, owner, owner_window, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            workbench.update(cx, |view, cx| match result {
                Ok(handle) => view.settings_window = Some(handle),
                Err(_) => {
                    view.error =
                        Some(crate::text::t("native.workbench.settingsWindowFailed").into());
                    cx.notify();
                }
            });
        });
    }
}

struct SettingsWindow {
    panel: Entity<SettingsPanel>,
    owner: WeakEntity<Workbench>,
    owner_window: AnyWindowHandle,
    _subscriptions: Vec<Subscription>,
}

impl SettingsWindow {
    fn new(
        panel: Entity<SettingsPanel>,
        owner: WeakEntity<Workbench>,
        owner_window: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = vec![cx.observe(&panel, |_, _, cx| cx.notify())];
        subscriptions.push(cx.subscribe(&panel, |view, _, event, cx| match event {
            SettingsEvent::OpenDocument { project, path } => {
                let project = project.clone();
                let path = path.clone();
                view.in_workbench(cx, move |view, window, cx| {
                    if view
                        .project
                        .as_ref()
                        .is_some_and(|current| current.identity == project)
                    {
                        view.open_document(path, None, window, cx);
                    }
                });
            }
        }));
        if let Some(owner) = owner.upgrade() {
            subscriptions.push(cx.observe(&owner, |view, _, cx| view.sync_project(cx)));
        }
        panel.update(cx, |panel, cx| panel.ensure_loaded(cx));
        cx.defer_in(window, |view, window, cx| {
            view.sync_project(cx);
            window.focus(&view.panel.read(cx).focus_handle(cx), cx);
        });
        Self {
            panel,
            owner,
            owner_window,
            _subscriptions: subscriptions,
        }
    }

    fn sync_project(&self, cx: &mut Context<Self>) {
        let project = self.owner.upgrade().and_then(|owner| {
            owner
                .read(cx)
                .project
                .as_ref()
                .map(|project| (project.identity.clone(), project.index.clone()))
        });
        self.panel
            .update(cx, |panel, cx| panel.bind_knowledge_project(project, cx));
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
            crate::text::t("native.workbench.settingsWindowDirtyTitle")
        } else {
            crate::text::t("native.workbench.settingsWindowTitle")
        };
        window.set_window_title(&title);
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
            .on_action(cx.listener(|view, command: &MenuCommand, window, cx| {
                if let MenuCommand::Edit(edit) = command {
                    window.dispatch_action(edit.text_action(), cx);
                    return;
                }
                let command = command.clone();
                view.in_workbench(cx, move |view, window, cx| {
                    view.dispatch_menu(&command, window, cx);
                });
            }))
            .child(window_chrome::title_bar(
                cx.listener(|view, _, window, cx| {
                    if !view.panel.read(cx).has_pending_operation() {
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
