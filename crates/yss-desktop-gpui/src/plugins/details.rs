use super::{DetailTab, PluginsPanel, commands::PluginAction};
use crate::appearance;
use gpui::{AnyElement, ClipboardItem, Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit_assets::IconName;
use yss_plugin_runtime::{InstalledPlugin, TaskSnapshot, TaskState, ViewScope};

fn task_state(state: TaskState) -> &'static str {
    match state {
        TaskState::Admitted => "已接纳",
        TaskState::Running => "运行中",
        TaskState::CancelRequested => "取消中",
        TaskState::Succeeded => "已完成",
        TaskState::Failed => "失败",
        TaskState::Cancelled => "已取消",
        TaskState::OutcomeUnknown => "结果未确认",
    }
}
fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / 1048576.)
}
impl PluginsPanel {
    pub(super) fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(plugin) = self.selected_plugin() else {
            return appearance::empty_state(
                IconName::Puzzle,
                "选择一个插件",
                "查看插件信息、运行状态、存储和诊断",
                cx,
            )
            .into_any_element();
        };
        let key = super::PluginKey::from_plugin(plugin);
        let toggle = key.clone();
        let enabled = plugin.enabled;
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(plugin.manifest.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} · {} · {}",
                                        plugin.manifest.version,
                                        plugin.manifest.publisher,
                                        match plugin.process_state.as_str() {
                                            "running" => "正在运行",
                                            "crashed" => "进程已退出",
                                            "stopped" => "尚未运行",
                                            _ => "状态待确认",
                                        }
                                    )),
                            ),
                    )
                    .child(
                        Button::new("plugin-enable")
                            .small()
                            .label(if enabled { "停用" } else { "启用" })
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.request_action(
                                    Some(toggle.clone()),
                                    PluginAction::Enable(!enabled),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("plugin-uninstall")
                            .small()
                            .ghost()
                            .label("卸载…")
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.request_action(
                                    Some(key.clone()),
                                    PluginAction::Uninstall,
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
            .child(self.render_tabs(cx))
            .child(div().flex_1().min_h_0().child(match self.tab {
                DetailTab::Overview => self.render_overview(plugin, cx),
                DetailTab::Tasks => self.render_tasks(cx),
                DetailTab::Diagnostics => self.render_diagnostics(cx),
            }))
            .into_any_element()
    }
    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut tabs = div()
            .flex()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border);
        for (id, label, tab) in [
            ("plugin-overview", "概览", DetailTab::Overview),
            ("plugin-tasks", "任务历史", DetailTab::Tasks),
            ("plugin-diagnostics", "诊断", DetailTab::Diagnostics),
        ] {
            tabs = tabs.child(
                Button::new(id)
                    .small()
                    .ghost()
                    .label(label)
                    .selected(self.tab == tab)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.tab = tab;
                        cx.notify();
                    })),
            );
        }
        tabs
    }
    fn render_overview(&self, plugin: &InstalledPlugin, cx: &mut Context<Self>) -> AnyElement {
        let mut view = div()
            .id("plugin-overview-body")
            .size_full()
            .p_4()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_sm().child(plugin.manifest.description.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "插件 ID：{}\n目标平台：{}\n签名指纹：{}",
                        plugin.manifest.id, plugin.manifest.target, plugin.signer_key
                    )),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("声明权限"),
            )
            .child(
                div()
                    .text_xs()
                    .child(if plugin.manifest.permissions.is_empty() {
                        "无声明权限".into()
                    } else {
                        plugin.manifest.permissions.join("、")
                    }),
            );
        if let Some(detail) = &self.detail {
            let key = detail.key.clone();
            view = view
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("插件存储"),
                )
                .child(div().text_sm().child(format!(
                    "已用 {} / 预算 {} · 缓存 {}",
                    mib(detail.storage.used_bytes),
                    mib(detail.storage.budget_bytes),
                    mib(detail.storage.cache_bytes)
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("这是应用预算限制，原生程序的系统文件访问不受该预算隔离。"),
                )
                .child(div().text_xs().child(format!(
                    "缓存目录：{}",
                    if plugin.manifest.cache_directories.is_empty() {
                        "未声明".into()
                    } else {
                        plugin.manifest.cache_directories.join("、")
                    }
                )))
                .child(
                    Button::new("plugin-clear-cache")
                        .small()
                        .label("清理缓存…")
                        .disabled(self.busy() || plugin.manifest.cache_directories.is_empty())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            view.request_action(
                                Some(key.clone()),
                                PluginAction::ClearCache,
                                window,
                                cx,
                            )
                        })),
                );
        } else {
            view = view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.busy() {
                        "正在读取存储信息…"
                    } else {
                        "存储信息暂不可用，请刷新。"
                    }),
            );
        }
        view = view.child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("贡献能力"),
        );
        for contribution in &plugin.manifest.contributes.views {
            view = view.child(div().text_xs().child(format!(
                "视图：{} · {}",
                contribution.title,
                if contribution.scope == ViewScope::Project {
                    "项目"
                } else {
                    "应用"
                }
            )));
        }
        if !plugin.manifest.contributes.views.is_empty() {
            view = view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("当前原生版本尚不能打开此插件提供的自定义界面。"),
            );
        }
        for command in &plugin.manifest.contributes.commands {
            view = view.child(div().text_xs().child(format!("命令：{}", command.title)));
        }
        view.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "授权预算：{} 个并发任务 · {} 个视图 · 数据交换 {}",
                    plugin.granted_budget.active_tasks,
                    plugin.granted_budget.views,
                    mib(plugin.granted_budget.snapshot_bytes)
                )),
        )
        .into_any_element()
    }
    fn render_tasks(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(detail) = &self.detail else {
            return div()
                .p_4()
                .text_sm()
                .child("任务信息暂不可用，请刷新。")
                .into_any_element();
        };
        let key = detail.key.clone();
        let active = detail.active.clone();
        let history = detail.history.tasks.clone();
        let count = active.len() + history.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("最近 30 天。清理只移除结果缓存，任务状态与项目结果会保留。"),
                    )
                    .child(
                        Button::new("plugin-history-clear")
                            .small()
                            .ghost()
                            .label("清理结果缓存…")
                            .disabled(self.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.request_action(
                                    Some(key.clone()),
                                    PluginAction::ClearHistory,
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
            .child(if count == 0 {
                appearance::empty_state(
                    IconName::List,
                    "暂无任务记录",
                    "插件任务的运行与终态由插件服务记录",
                    cx,
                )
                .into_any_element()
            } else {
                uniform_list(
                    "plugin-task-list",
                    count,
                    cx.processor(move |view, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| {
                                if index < active.len() {
                                    active.get(index)
                                } else {
                                    history.get(index - active.len())
                                }
                            })
                            .map(|task| view.render_task(task, cx).into_any_element())
                            .collect()
                    }),
                )
                .flex_1()
                .into_any_element()
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("plugin-history-previous")
                            .small()
                            .ghost()
                            .label("上一页")
                            .disabled(self.busy() || self.cursor_stack.len() <= 1)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.history_page(false, window, cx)
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(format!("第 {} 页", self.cursor_stack.len().max(1))),
                    )
                    .child(
                        Button::new("plugin-history-next")
                            .small()
                            .ghost()
                            .label("下一页")
                            .disabled(self.busy() || detail.history.next_cursor.is_none())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.history_page(true, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
    fn render_task(&self, task: &TaskSnapshot, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = task.task_id.clone();
        div()
            .h(px(72.))
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .truncate()
                            .child(task.task_id.clone()),
                    )
                    .child(div().text_xs().child(task_state(task.state)))
                    .child(
                        Button::new(gpui::SharedString::from(format!("task-copy-{id}")))
                            .small()
                            .ghost()
                            .label("复制 ID")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(id.clone()))
                            }),
                    ),
            )
            .when_some(task.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .truncate()
                        .child(super::failure(error)),
                )
            })
    }
    fn render_diagnostics(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(detail) = &self.detail else {
            return div()
                .p_4()
                .text_sm()
                .child("诊断暂不可用，请刷新。")
                .into_any_element();
        };
        let truncated = detail.diagnostics.iter().any(|entry| entry.truncated);
        let diagnostics = detail.diagnostics.clone();
        let count = detail.diagnostic_lines.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if truncated {
                                "进程诊断保存在本机；较早内容已超出缓冲容量。"
                            } else {
                                "进程诊断仅在本机显示。"
                            }),
                    )
                    .child(
                        Button::new("plugin-diagnostics-copy")
                            .small()
                            .ghost()
                            .label("复制诊断")
                            .disabled(count == 0)
                            .on_click(move |_, _, cx| {
                                let text = diagnostics
                                    .iter()
                                    .map(|entry| {
                                        format!(
                                            "[{} {}]\n{}",
                                            entry.instance_id,
                                            entry.task_ids.join(", "),
                                            entry.stderr
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }),
                    ),
            )
            .child(if count == 0 {
                appearance::empty_state(
                    IconName::File,
                    "暂无诊断",
                    "进程启动后会保留有界的标准错误输出",
                    cx,
                )
                .into_any_element()
            } else {
                uniform_list(
                    "plugin-diagnostic-lines",
                    count,
                    cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                        let Some(detail) = &view.detail else {
                            return vec![];
                        };
                        range
                            .filter_map(|index| detail.diagnostic_lines.get(index))
                            .map(|line| {
                                div()
                                    .h(px(22.))
                                    .px_4()
                                    .text_xs()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .truncate()
                                    .child(line.clone())
                                    .into_any_element()
                            })
                            .collect()
                    }),
                )
                .flex_1()
                .into_any_element()
            })
            .into_any_element()
    }
}
