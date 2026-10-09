use super::{
    CancelResponse, ConversationPanel,
    projection::{Task, Turn, TurnState},
};
use gpui::{
    AnyElement, Context, Empty, IntoElement, Render, SharedString, Window, div, prelude::*,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    text::TextView,
};
use yss_harness_contract::{AgentRole, ModelCallPurpose};

impl Render for ConversationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_timing(window, cx);
        self.sync_input(window, cx);
        div()
            .id("assistant-conversation")
            .key_context("AssistantConversation")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &CancelResponse, _, cx| view.cancel(cx)))
            .child(self.render_thread(cx))
            .child(self.status(cx))
            .child(self.render_composer(window, cx))
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
        if error.is_none()
            && !self.refreshing
            && self.model_available()
            && !self.running()
            && !self.stopping
        {
            return Empty.into_any_element();
        }
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
                crate::text::t("panel.assistantStatusReady").into()
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
                .child(
                    self.reference_chips(
                        format!("history-references-{}", turn.id),
                        turn.resources
                            .iter()
                            .map(|reference| (&reference.resource, Some(reference.name.as_str()))),
                        false,
                        cx,
                    ),
                ),
        );
        item = item.child(self.execution_header(turn, cx));
        if !turn.reasoning.is_empty() {
            let key = format!("reasoning-{id}");
            let expanded = self.expanded.get(&key).copied().unwrap_or(false);
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
                        view.expanded.insert(key.clone(), !expanded);
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
        item = item.child(self.tool_group(
            format!("turn-tools-{}", turn.id),
            turn.tools.iter(),
            turn.state == TurnState::Running,
            cx,
        ));
        for task in turn
            .tasks
            .iter()
            .filter(|task| task.role != AgentRole::Manager)
        {
            item = item.child(self.task_card(
                task.sequence,
                task,
                turn.state == TurnState::Running,
                cx,
            ));
        }
        if let Some(plan) = &turn.plan {
            item = item.child(self.plan_card(format!("plan-{id}"), plan, cx));
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
        let mut citations = div().min_w_0().flex().flex_col().gap_2();
        for (index, citation) in turn.citations.iter().enumerate() {
            citations = citations.child(super::sources::card(
                format!("citation-{id}-{index}"),
                citation,
                cx.entity().downgrade(),
            ));
        }
        item.child(citations).into_any_element()
    }

    pub(super) fn task_content(
        &self,
        id: u64,
        task: &Task,
        running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.tool_group(format!("task-tools-{id}"), task.tools.iter(), running, cx));
        if let Some(summary) = &task.summary
            && !summary.is_empty()
        {
            body = body.child(TextView::markdown(
                SharedString::from(format!("task-summary-{id}")),
                summary.clone(),
            ));
        }
        if let Some(plan) = &task.plan {
            body = body.child(self.plan_card(format!("task-plan-{id}"), plan, cx));
        }
        for warning in &task.warnings {
            body = body.child(div().text_xs().child(warning.clone()));
        }
        body.into_any_element()
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
