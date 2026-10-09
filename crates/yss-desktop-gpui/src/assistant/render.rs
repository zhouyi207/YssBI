use super::{
    CancelResponse, ConversationEvent, ConversationPanel, SendMessage,
    projection::{Task, Tool, Turn, TurnState},
};
use gpui::{AnyElement, Context, IntoElement, Render, SharedString, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    text::TextView,
};
use yss_harness_contract::{AgentRole, AgentRunState, ModelCallPurpose};

impl Render for ConversationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_timing(window, cx);
        div()
            .id("assistant-conversation")
            .key_context("AssistantConversation")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &SendMessage, window, cx| view.send(window, cx)))
            .on_action(cx.listener(|view, _: &CancelResponse, _, cx| view.cancel(cx)))
            .child(self.render_thread(cx))
            .child(self.status(cx))
            .child(self.render_composer(cx))
    }
}
impl ConversationPanel {
    fn status(&self, cx: &mut Context<Self>) -> AnyElement {
        let latest = self.transcript.turns.last();
        let error = self
            .stream_error
            .clone()
            .or_else(|| self.error.clone())
            .or_else(|| {
                latest
                    .and_then(|turn| turn.error.as_deref())
                    .map(super::commands::failure_text)
            });
        let text = error.clone().unwrap_or_else(|| {
            if self.refreshing {
                "正在恢复会话…".into()
            } else if !self.model_available() {
                "请在模型设置中配置可用的模型。".into()
            } else if self.stopping {
                "已请求停止，等待正在执行的操作结束…".into()
            } else if self.running() {
                latest
                    .and_then(|turn| turn.activity.clone())
                    .unwrap_or_else(|| "正在处理…".into())
            } else {
                "就绪 · Ctrl+Enter 发送".into()
            }
        });
        div()
            .px_3()
            .py_1()
            .text_xs()
            .flex_shrink_0()
            .text_color(if error.is_some() {
                cx.theme().danger
            } else {
                cx.theme().muted_foreground
            })
            .child(text)
            .into_any_element()
    }
    pub(super) fn render_turn(&self, turn: &Turn, cx: &mut Context<Self>) -> AnyElement {
        let text = turn.user.clone();
        let copy_user = text.clone();
        let id = &turn.id;
        let mut item = div().flex().flex_col().gap_3().min_w_0().child(
            div()
                .p_3()
                .rounded_lg()
                .bg(cx.theme().muted)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("你"),
                        )
                        .child(
                            Button::new(SharedString::from(format!("copy-user-{id}")))
                                .small()
                                .ghost()
                                .label("复制")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.copy_message(copy_user.clone(), cx)
                                })),
                        ),
                )
                .child(TextView::markdown(
                    SharedString::from(format!("user-{id}")),
                    text,
                ))
                .child(self.resource_chips(turn, cx)),
        );
        item = item.child(self.execution_header(turn, cx));
        if !turn.reasoning.is_empty() {
            let key = format!("reasoning-{id}");
            let expanded = self.expanded.contains(&key);
            item = item.child(
                Button::new(SharedString::from(key.clone()))
                    .small()
                    .ghost()
                    .label(if expanded {
                        "收起推理文本"
                    } else {
                        "查看推理文本"
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.expanded.insert(key.clone()) {
                            view.expanded.remove(&key);
                        }
                        cx.notify();
                    })),
            );
            if expanded {
                item = item.child(div().p_3().rounded_md().bg(cx.theme().muted).child(
                    TextView::markdown(
                        SharedString::from(format!("reasoning-text-{id}")),
                        turn.reasoning.clone(),
                    ),
                ));
            }
        }
        item = item.child(self.tool_list(&turn.tools, turn.state == TurnState::Running, cx));
        for task in turn
            .tasks
            .values()
            .filter(|task| task.role != AgentRole::Manager)
        {
            item = item.child(self.task_card(task, turn.state == TurnState::Running, cx));
        }
        if let Some(plan) = &turn.plan {
            item = item.child(
                div()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().child("统计计划"))
                    .child(TextView::markdown(
                        SharedString::from(format!("plan-{id}")),
                        format!(
                            "**研究问题**\n\n{}\n\n**设计**\n\n{}\n\n**方法**\n\n{}",
                            plan.research_question,
                            plan.study_design.description,
                            plan.candidate_methods
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("、")
                        ),
                    )),
            );
        }
        if !turn.text.is_empty() {
            item = item.child(TextView::markdown(
                SharedString::from(format!("assistant-{id}")),
                turn.text.clone(),
            ));
        }
        let copy_reply = turn.text.clone();
        item = item
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match turn.state {
                        TurnState::Running => "生成中",
                        TurnState::Completed => "已完成",
                        TurnState::Failed => "已中断",
                        TurnState::Cancelled => "已停止",
                    })
                    .child(
                        Button::new(SharedString::from(format!("copy-reply-{id}")))
                            .small()
                            .ghost()
                            .label("复制回复")
                            .disabled(turn.text.is_empty())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.copy_message(copy_reply.clone(), cx)
                            })),
                    ),
            )
            .child(self.usage(turn, cx));
        if let Some(error) = &turn.error {
            item = item.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(super::commands::failure_text(error)),
            );
        }
        if matches!(turn.state, TurnState::Failed | TurnState::Cancelled) {
            item = item.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("已完成的项目操作仍然保留。"),
            );
        }
        let mut citations = div().flex().flex_wrap().gap_1();
        for (index, citation) in turn.citations.iter().enumerate() {
            let citation = citation.clone();
            citations = citations.child(
                Button::new(SharedString::from(format!("citation-{id}-{index}")))
                    .small()
                    .ghost()
                    .label(citation.title.clone())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.inspect_citation(citation.clone(), window, cx)
                    })),
            );
        }
        item.child(citations).into_any_element()
    }
    fn resource_chips(&self, turn: &Turn, cx: &mut Context<Self>) -> AnyElement {
        let mut chips = div().flex().flex_wrap().gap_1();
        for (index, reference) in turn.resources.iter().enumerate() {
            let resource = reference.resource.clone();
            chips = chips.child(
                Button::new(SharedString::from(format!(
                    "history-resource-{}-{index}",
                    turn.id
                )))
                .small()
                .ghost()
                .label(reference.name.clone())
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(ConversationEvent::OpenResource(resource.clone()))
                })),
            );
        }
        chips.into_any_element()
    }
    fn tool_list(&self, tools: &[Tool], running: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div().flex().flex_col().gap_1();
        for tool in tools {
            let id = tool.id.clone();
            let code = match tool.kind {
                yss_harness_contract::AssistantToolIdentity::Capability(kind) => {
                    kind.as_str().to_owned()
                }
                yss_harness_contract::AssistantToolIdentity::Control(kind) => {
                    kind.as_str().to_owned()
                }
            };
            let key = format!("panel.assistantToolNames.{code}");
            let title = crate::text::translate(&key);
            let title = if title == key {
                "工具操作".into()
            } else {
                title
            };
            let state = if tool.failure.is_some() {
                crate::text::t("panel.assistantToolFailed")
            } else if tool.finished {
                crate::text::t("panel.assistantToolCompleted")
            } else if !running {
                crate::text::t("panel.assistantToolInterrupted")
            } else if !self.timing_connected() {
                crate::text::t("panel.assistantToolUnknown")
            } else {
                crate::text::t("panel.assistantToolRunning")
            };
            list = list.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!("tool-{id}")))
                            .small()
                            .ghost()
                            .label(format!(
                                "{title} · {state}{}",
                                tool.execution
                                    .as_ref()
                                    .map(|status| format!(" · {status}"))
                                    .unwrap_or_default()
                            ))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.inspect_tool(id.clone(), window, cx)
                            })),
                    )
                    .child(self.elapsed(
                        format!("tool-time-{}", tool.id),
                        tool.timing,
                        running && !tool.finished,
                        cx,
                    )),
            );
        }
        list.into_any_element()
    }
    fn task_card(&self, task: &Task, running: bool, cx: &mut Context<Self>) -> AnyElement {
        let key = format!("task-{}", task.id);
        let expanded = self.expanded.contains(&key);
        let state = match task.state {
            None if !running => crate::text::t("panel.assistantToolInterrupted"),
            None if !self.timing_connected() => crate::text::t("panel.assistantToolUnknown"),
            None => crate::text::t("panel.assistantToolRunning"),
            Some(AgentRunState::Completed) => "完成",
            Some(AgentRunState::Stale) => "已过期",
            Some(AgentRunState::Blocked) => "受阻",
            Some(AgentRunState::Failed) => "失败",
            Some(AgentRunState::Cancelled) => "已取消",
            Some(AgentRunState::Interrupted) => "已中断",
        };
        let mut card = div()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(
                Button::new(SharedString::from(key.clone()))
                    .small()
                    .ghost()
                    .label(format!(
                        "{} · {state} · {}",
                        task.role.name(),
                        task.objective
                    ))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.expanded.insert(key.clone()) {
                            view.expanded.remove(&key);
                        }
                        cx.notify();
                    })),
            )
            .child(self.elapsed(
                format!("task-time-{}", task.id),
                Some(task.timing),
                running && task.state.is_none(),
                cx,
            ));
        if expanded {
            if let Some(summary) = &task.summary {
                card = card.child(TextView::markdown(
                    SharedString::from(format!("task-summary-{}", task.id)),
                    summary.clone(),
                ));
            }
            if let Some(error) = &task.error {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(super::commands::failure_text(error)),
                );
            }
            for warning in &task.warnings {
                card = card.child(div().text_xs().child(warning.clone()));
            }
            card = card.child(self.tool_list(&task.tools, running && task.state.is_none(), cx));
            for (index, artifact) in task.artifacts.iter().enumerate() {
                let resource = artifact.resource.clone();
                card = card.child(
                    Button::new(SharedString::from(format!(
                        "task-artifact-{}-{index}",
                        task.id
                    )))
                    .small()
                    .ghost()
                    .label(resource.id.clone())
                    .disabled(artifact.deleted)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(ConversationEvent::OpenResource(resource.clone()))
                    })),
                );
            }
            for (index, result) in task.results.iter().enumerate() {
                let result = result.clone();
                card = card.child(
                    Button::new(SharedString::from(format!(
                        "task-result-{}-{index}",
                        task.id
                    )))
                    .small()
                    .ghost()
                    .label(result.output.clone())
                    .on_click(cx.listener(move |view, _, _, cx| view.open_result(&result, cx))),
                );
            }
        }
        card.into_any_element()
    }
    fn usage(&self, turn: &Turn, cx: &mut Context<Self>) -> AnyElement {
        let Some((usage, context, _)) = turn
            .usage
            .iter()
            .rev()
            .find(|(_, _, purpose)| *purpose == ModelCallPurpose::Response)
        else {
            return div().into_any_element();
        };
        let count = |value: Option<u64>| {
            value
                .map(|n| n.to_string())
                .unwrap_or_else(|| "未知".into())
        };
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
                "最近调用 · 输入 {} · 输出 {} · 缓存读 {} · 缓存写 {} · 推理 {}{}",
                count(usage.input_tokens),
                count(usage.output_tokens),
                count(usage.cached_input_tokens),
                count(usage.cache_creation_input_tokens),
                count(usage.reasoning_tokens),
                context
                    .map(|n| format!(" · 上下文容量 {n}"))
                    .unwrap_or_default()
            ))
            .into_any_element()
    }
}
