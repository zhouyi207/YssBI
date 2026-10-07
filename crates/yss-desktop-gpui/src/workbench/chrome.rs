//! Workbench window chrome; DockArea retains every panel's layout and activation.
use super::Workbench;
use crate::{
    appearance,
    assets::NativeIcon,
    canvas::{CancelRun, GraphCommand, RunWholeGraph, SaveGraph},
};
use gpui::{Context, IntoElement, MouseButton, actions, div, prelude::*, px, rgb};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, TitleBar,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};

actions!(native_workbench, [SaveAllGraphs]);

impl Workbench {
    pub(super) fn render_chrome(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dirty = self
            .graphs
            .values()
            .filter_map(gpui::WeakEntity::upgrade)
            .filter(|graph| graph.read(cx).dirty())
            .count();
        let name = self
            .project
            .as_ref()
            .map(|project| project.index.project_name.clone())
            .unwrap_or_else(|| "工作台".into());
        div()
            .key_context("Workbench")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|view, _: &SaveGraph, _, cx| {
                if !view.busy
                    && !view.closing
                    && let Some(graph) = view.details.read(cx).graph()
                {
                    graph.update(cx, |graph, cx| graph.submit(GraphCommand::Save, None, cx));
                }
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
                    view.save_all(false, window, cx);
                }
            }))
            .child(
                TitleBar::new()
                    .h(px(38.))
                    .on_close_window(cx.listener(|view, _, window, cx| {
                        if view.close_requested(window, cx) {
                            window.remove_window();
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .min_w_0()
                            .child(
                                Icon::new(NativeIcon::Graph)
                                    .size_4()
                                    .text_color(rgb(appearance::BLUE)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("YssBI"),
                            )
                            .child(div().h_4().w(px(1.)).mx_2().bg(cx.theme().border))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(
                                        Button::new("new-graph").small().ghost().icon(IconName::Plus).tooltip("新建图")
                                            .disabled(self.busy || self.closing || self.project.is_none())
                                            .dropdown_menu({
                                                let view = cx.entity().downgrade();
                                                move |menu, _, _| {
                                                    let event_view = view.clone();
                                                    let function_view = view.clone();
                                                    menu.item(PopupMenuItem::new("新建事件图").on_click(move |_, window, cx| {
                                                        let _ = event_view.update(cx, |view, cx| view.create_graph(yss_graph_document::GraphResourceKind::EventGraph, window, cx));
                                                    }))
                                                    .item(PopupMenuItem::new("新建函数图").on_click(move |_, window, cx| {
                                                        let _ = function_view.update(cx, |view, cx| view.create_graph(yss_graph_document::GraphResourceKind::FunctionGraph, window, cx));
                                                    }))
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("open-project")
                                            .small()
                                            .ghost()
                                            .icon(IconName::FolderOpen)
                                            .label("打开项目")
                                            .disabled(self.busy || self.closing)
                                            .on_click(cx.listener(Self::choose_project)),
                                    )
                                    .child(
                                        Button::new("save-all")
                                            .small()
                                            .ghost()
                                            .icon(NativeIcon::Save)
                                            .tooltip("保存所有更改 · Ctrl+Shift+S")
                                            .disabled(self.busy || self.closing || dirty == 0)
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.save_all(false, window, cx)
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(self.dock.clone())
                    .when(self.project.is_none(), |view| {
                        view.child(self.render_welcome(cx))
                    })
                    .when(self.busy || self.closing, |view| {
                        view.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(gpui::rgba(0x141820cc))
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
            .child(
                div()
                    .h(px(26.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().title_bar)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().size(px(5.)).rounded_full().bg(rgb(if dirty > 0 {
                        appearance::AMBER
                    } else {
                        appearance::GREEN
                    })))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .when(self.error.is_some(), |view| {
                                view.text_color(cx.theme().danger)
                            })
                            .child(self.error.clone().unwrap_or_else(|| {
                                if self.busy {
                                    "正在打开项目…".into()
                                } else if dirty > 0 {
                                    format!("{dirty} 个文件有未保存的更改")
                                } else if self.project.is_some() {
                                    "所有更改已保存".into()
                                } else {
                                    "选择项目以开始".into()
                                }
                            })),
                    )
                    .child("Ctrl+S 保存")
                    .child(div().w(px(1.)).h_3().mx_2().bg(cx.theme().border))
                    .child("Ctrl+Z 撤销"),
            )
    }

    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .absolute()
            .inset_0()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .size_16()
                    .rounded_lg()
                    .bg(cx.theme().muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(NativeIcon::Graph)
                            .size_8()
                            .text_color(rgb(appearance::BLUE)),
                    ),
            )
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("YssBI"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("连接数据，让分析清晰可见"),
            )
            .child(
                Button::new("welcome-open")
                    .primary()
                    .icon(IconName::FolderOpen)
                    .label("打开项目")
                    .on_click(cx.listener(Self::choose_project)),
            )
    }
}
