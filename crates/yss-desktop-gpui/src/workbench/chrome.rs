//! Workbench window chrome; DockArea retains every panel's layout and activation.
use super::{
    Workbench,
    menus::{MenuCommand, WorkbenchPanel},
};
use crate::{
    appearance,
    assets::NativeIcon,
    canvas::{CancelRun, RunWholeGraph, SaveGraph},
    window_chrome,
};
use gpui::{Context, IntoElement, MouseButton, Window, actions, div, prelude::*, px, rgb};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
};

actions!(
    native_workbench,
    [
        SaveAllGraphs,
        ShowSettings,
        OpenProjectDirectory,
        OpenRecentProject
    ]
);

impl Workbench {
    pub(super) fn render_chrome(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let dirty_files = self
            .graphs
            .values()
            .filter_map(gpui::WeakEntity::upgrade)
            .filter(|graph| graph.read(cx).dirty())
            .count()
            + self
                .documents
                .values()
                .filter_map(gpui::WeakEntity::upgrade)
                .filter(|document| document.read(cx).dirty())
                .count()
            + self
                .minds
                .values()
                .filter_map(gpui::WeakEntity::upgrade)
                .filter(|mind| mind.read(cx).dirty())
                .count();
        let pending_databases = self
            .databases
            .values()
            .filter_map(gpui::WeakEntity::upgrade)
            .filter(|view| view.read(cx).dirty())
            .count();
        let dirty_files = dirty_files
            + self
                .charts
                .values()
                .filter_map(gpui::WeakEntity::upgrade)
                .filter(|view| view.read(cx).dirty())
                .count();
        let dirty = dirty_files + pending_databases + usize::from(self.settings.read(cx).dirty());
        let name = self
            .project
            .as_ref()
            .map(|project| project.index.project_name.clone())
            .unwrap_or_else(|| "工作台".into());
        let status = self.error.clone().unwrap_or_else(|| {
            if self.busy {
                "正在处理项目…".into()
            } else if dirty_files > 0 && pending_databases > 0 {
                format!("{dirty_files} 个文件待保存 · {pending_databases} 个数据库待保存检查点")
            } else if pending_databases > 0 {
                format!("{pending_databases} 个数据库待保存检查点")
            } else if dirty_files > 0 {
                format!("{dirty_files} 个文件有未保存的更改")
            } else if self.settings.read(cx).dirty() {
                "设置有未保存的更改".into()
            } else if self.project.is_some() {
                "所有更改已保存".into()
            } else {
                "打开项目目录以开始".into()
            }
        });
        div()
            .key_context("Workbench")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|view, _: &SaveGraph, window, cx| {
                view.save_current(window, cx);
            }))
            .on_action(cx.listener(|view, _: &ShowSettings, window, cx| {
                view.show_panel(WorkbenchPanel::Settings, window, cx);
            }))
            .on_action(cx.listener(|view, _: &OpenProjectDirectory, window, cx| {
                view.choose_project(window, cx);
            }))
            .on_action(cx.listener(|view, _: &OpenRecentProject, window, cx| {
                view.open_recent_projects(window, cx);
            }))
            .on_action(cx.listener(|view, _: &RunWholeGraph, _, cx| {
                if !view.busy
                    && !view.closing
                    && let Some(graph) = view.details.read(cx).graph()
                {
                    graph.update(cx, |graph, cx| {
                        graph.run_graph(yss_application::graph::run::RunDemand::Default, cx)
                    });
                }
            }))
            .on_action(cx.listener(|view, _: &CancelRun, _, cx| {
                if !view.busy
                    && !view.closing
                    && let Some(graph) = view.details.read(cx).graph()
                {
                    graph.update(cx, |graph, cx| graph.cancel_run(cx));
                }
            }))
            .on_action(cx.listener(|view, _: &SaveAllGraphs, window, cx| {
                if !view.busy && !view.closing {
                    view.save_all(super::lifecycle::AfterSave::Stay, window, cx);
                }
            }))
            .on_action(cx.listener(|view, command: &MenuCommand, window, cx| {
                view.dispatch_menu(command, window, cx);
            }))
            .child(window_chrome::title_bar(
                cx.listener(|view, _, window, cx| {
                    if view.close_requested(window, cx) {
                        window.remove_window();
                    }
                }),
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(NativeIcon::Graph)
                                    .size_4()
                                    .text_color(rgb(appearance::BLUE)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("YssBI"),
                            ),
                    )
                    .when(!cfg!(target_os = "macos"), |view| {
                        view.child(
                            div()
                                .w(px(120.))
                                .h_full()
                                .flex_shrink_0()
                                .child(self.menu_bar.clone()),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(name),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .pr_2()
                            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                                window.prevent_default();
                                cx.stop_propagation();
                            })
                            .child(
                                Button::new("save-all")
                                    .small()
                                    .ghost()
                                    .icon(NativeIcon::Save)
                                    .tooltip("保存所有更改 · Ctrl+Shift+S")
                                    .disabled(self.is_closing(cx) || dirty == 0)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.save_all(super::lifecycle::AfterSave::Stay, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("title-settings")
                                    .small()
                                    .ghost()
                                    .icon(IconName::Settings)
                                    .tooltip("设置 · Ctrl+,")
                                    .disabled(self.is_closing(cx))
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.show_settings(window, cx)
                                    })),
                            ),
                    ),
                window,
                cx,
            ))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(self.dock.clone())
                    .when(self.project.is_none(), |view| {
                        view.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(cx.theme().background)
                                .child(self.render_welcome(window, cx)),
                        )
                    })
                    .when(self.busy || self.closing, |view| {
                        view.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(cx.theme().overlay)
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::LoaderCircle)
                                        .size_4()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child("正在处理…"),
                        )
                    }),
            )
            .child(self.render_status_bar(status, dirty > 0, cx))
    }
}
