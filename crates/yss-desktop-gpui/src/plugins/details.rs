use super::{DetailTab, PluginsPanel, commands::PluginAction};
use crate::appearance;
use gpui::{AnyElement, ClipboardItem, Context, IntoElement, div, prelude::*, px, uniform_list};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit_assets::IconName;
use yss_plugin_runtime::{InstalledPlugin, TaskSnapshot, TaskState, ViewScope};

pub(super) fn task_state(state: TaskState) -> &'static str {
    match state {
        TaskState::Admitted => crate::text::t("native.plugins.accepted"),
        TaskState::Running => crate::text::t("common.running"),
        TaskState::CancelRequested => crate::text::t("native.plugins.cancelling"),
        TaskState::Succeeded => crate::text::t("common.completed"),
        TaskState::Failed => crate::text::t("panel.assistantToolFailed"),
        TaskState::Cancelled => crate::text::t("plugins.taskStates.cancelled"),
        TaskState::OutcomeUnknown => crate::text::t("native.plugins.outcomeUnknown"),
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
                crate::text::t("native.plugins.choosePlugin"),
                crate::text::t("native.plugins.detailsHint"),
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
                                            "running" => crate::text::t("native.plugins.running"),
                                            "crashed" => crate::text::t("native.plugins.exited"),
                                            "stopped" =>
                                                crate::text::t("canvas.graphState.unexecuted"),
                                            _ => crate::text::t("panel.assistantToolUnknown"),
                                        }
                                    )),
                            ),
                    )
                    .child(
                        Button::new("plugin-enable")
                            .small()
                            .label(if enabled {
                                crate::text::t("native.plugins.disable")
                            } else {
                                crate::text::t("plugins.enable")
                            })
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
                            .label(crate::text::t("native.plugins.uninstall"))
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
            (
                "plugin-overview",
                crate::text::t("panel.assistantToolValues.overview"),
                DetailTab::Overview,
            ),
            (
                "plugin-tasks",
                crate::text::t("native.plugins.taskHistory"),
                DetailTab::Tasks,
            ),
            (
                "plugin-diagnostics",
                crate::text::t("panel.assistantToolValues.diagnostics"),
                DetailTab::Diagnostics,
            ),
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
                    .child(crate::text::format(
                        "native.plugins.identityDetails",
                        &[
                            ("value0", plugin.manifest.id.to_string()),
                            ("value1", plugin.manifest.target.to_string()),
                            ("value2", plugin.signer_key.to_string()),
                        ],
                    )),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(crate::text::t("native.plugins.permissions")),
            )
            .child(
                div()
                    .text_xs()
                    .child(if plugin.manifest.permissions.is_empty() {
                        crate::text::t("native.plugins.noPermissions").into()
                    } else {
                        plugin
                            .manifest
                            .permissions
                            .join(crate::text::t("common.listSeparator"))
                    }),
            );
        if let Some(detail) = &self.detail {
            let key = detail.key.clone();
            view = view
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(crate::text::t("native.plugins.storage")),
                )
                .child(div().text_sm().child(crate::text::format(
                    "native.plugins.storageUsage",
                    &[
                        ("value0", mib(detail.storage.used_bytes).to_string()),
                        ("value1", mib(detail.storage.budget_bytes).to_string()),
                        ("value2", mib(detail.storage.cache_bytes).to_string()),
                    ],
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t("native.plugins.storageWarning")),
                )
                .child(
                    div().text_xs().child(crate::text::format(
                        "native.plugins.cacheDirectory",
                        &[(
                            "value0",
                            (if plugin.manifest.cache_directories.is_empty() {
                                crate::text::t("native.plugins.undeclared").into()
                            } else {
                                plugin
                                    .manifest
                                    .cache_directories
                                    .join(crate::text::t("common.listSeparator"))
                            })
                            .to_string(),
                        )],
                    )),
                )
                .child(
                    Button::new("plugin-clear-cache")
                        .small()
                        .label(crate::text::t("native.plugins.clearCache"))
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
                        crate::text::t("native.plugins.loadingStorage")
                    } else {
                        crate::text::t("native.plugins.storageUnavailable")
                    }),
            );
        }
        view = view.child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(crate::text::t("native.plugins.contributions")),
        );
        for (index, contribution) in plugin.manifest.contributes.views.iter().enumerate() {
            let key = super::PluginKey::from_plugin(plugin);
            let declaration = contribution.clone();
            view = view.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(crate::text::format(
                        "native.plugins.viewContribution",
                        &[
                            ("value0", contribution.title.to_string()),
                            (
                                "value1",
                                (if contribution.scope == ViewScope::Project {
                                    crate::text::t("activityBar.project")
                                } else {
                                    crate::text::t("log.domains.application")
                                })
                                .to_string(),
                            ),
                        ],
                    ))
                    .child(
                        Button::new(("plugin-open-view", index))
                            .small()
                            .label(crate::text::t("native.plugins.openView"))
                            .disabled(self.busy() || !plugin.enabled)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(super::OpenNativeView {
                                    key: key.clone(),
                                    view: declaration.clone(),
                                });
                            })),
                    ),
            );
        }
        for command in &plugin.manifest.contributes.commands {
            view = view.child(div().text_xs().child(crate::text::format(
                "native.plugins.commandContribution",
                &[("value0", command.title.to_string())],
            )));
        }
        view.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::format(
                    "native.plugins.authorizedBudget",
                    &[
                        ("value0", plugin.granted_budget.active_tasks.to_string()),
                        ("value1", plugin.granted_budget.views.to_string()),
                        (
                            "value2",
                            mib(plugin.granted_budget.snapshot_bytes).to_string(),
                        ),
                    ],
                )),
        )
        .into_any_element()
    }
    fn render_tasks(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(detail) = &self.detail else {
            return div()
                .p_4()
                .text_sm()
                .child(crate::text::t("native.plugins.tasksUnavailable"))
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
                            .child(crate::text::t("native.plugins.historyHint")),
                    )
                    .child(
                        Button::new("plugin-history-clear")
                            .small()
                            .ghost()
                            .label(crate::text::t("native.plugins.clearResultCache"))
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
                    crate::text::t("native.plugins.noTasks"),
                    crate::text::t("native.plugins.tasksHint"),
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
                            .label(crate::text::t("sourceInspector.previous"))
                            .disabled(self.busy() || self.cursor_stack.len() <= 1)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.history_page(false, window, cx)
                            })),
                    )
                    .child(div().text_xs().child(crate::text::format(
                        "native.plugins.page",
                        &[("value0", self.cursor_stack.len().max(1).to_string())],
                    )))
                    .child(
                        Button::new("plugin-history-next")
                            .small()
                            .ghost()
                            .label(crate::text::t("sourceInspector.next"))
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
                            .label(crate::text::t("native.plugins.copyId"))
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
                .child(crate::text::t("native.plugins.diagnosticsUnavailable"))
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
                                crate::text::t("native.plugins.diagnosticsTruncated")
                            } else {
                                crate::text::t("native.plugins.diagnosticsLocal")
                            }),
                    )
                    .child(
                        Button::new("plugin-diagnostics-copy")
                            .small()
                            .ghost()
                            .label(crate::text::t("native.plugins.copyDiagnostics"))
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
                    crate::text::t("native.plugins.noDiagnostics"),
                    crate::text::t("native.plugins.diagnosticsHint"),
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
