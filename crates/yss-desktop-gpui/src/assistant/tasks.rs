//! Task and plan cards borrow the transcript; only disclosure choices belong to the view.
use super::{ConversationPanel, projection::Task};
use gpui::{AnyElement, Context, Empty, IntoElement, SharedString, div, prelude::*};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    spinner::Spinner,
    text::TextView,
};
use gpui_kit_assets::IconName;
use yss_harness_contract::{AgentRole, AgentRunState, AnalysisMode, StatisticalPlan};

impl ConversationPanel {
    pub(super) fn task_card(
        &self,
        id: u64,
        task: &Task,
        turn_running: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let running = turn_running && task.state.is_none();
        let failed = matches!(
            task.state,
            Some(AgentRunState::Failed | AgentRunState::Blocked | AgentRunState::Interrupted)
        ) || (!turn_running && task.state.is_none());
        let connected = self.timing_connected();
        let key = format!("task-{id}");
        let open = self
            .expanded
            .get(&key)
            .copied()
            .unwrap_or(running || failed);
        let role = crate::text::t(match task.role {
            AgentRole::Manager => "panel.assistantAgentRoles.manager",
            AgentRole::Data => "panel.assistantAgentRoles.data",
            AgentRole::Stats => "panel.assistantAgentRoles.stats",
            AgentRole::Plot => "panel.assistantAgentRoles.plot",
            AgentRole::Report => "panel.assistantAgentRoles.report",
            AgentRole::Review => "panel.assistantAgentRoles.review",
        });
        let state = crate::text::t(match task.state {
            None if running && !connected => "panel.assistantToolUnknown",
            None if running => "panel.assistantAgentStates.running",
            None | Some(AgentRunState::Interrupted) => "panel.assistantAgentStates.interrupted",
            Some(AgentRunState::Completed) => "panel.assistantAgentStates.completed",
            Some(AgentRunState::Stale) => "panel.assistantAgentStates.stale",
            Some(AgentRunState::Blocked) => "panel.assistantAgentStates.blocked",
            Some(AgentRunState::Failed) => "panel.assistantAgentStates.failed",
            Some(AgentRunState::Cancelled) => "panel.assistantAgentStates.cancelled",
        });
        let header = Button::new(SharedString::from(key.clone()))
            .small()
            .ghost()
            .w_full()
            .h_auto()
            .icon(if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .accessibility_label(role)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when(running && connected, |row| {
                        row.child(Spinner::new().small().icon(IconName::LoaderCircle))
                    })
                    .child(role)
                    .child(
                        div()
                            .text_color(if failed {
                                cx.theme().danger
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(state),
                    ),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.expanded.insert(key.clone(), !open);
                cx.notify();
            }));
        let body = if open {
            self.task_content(id, task, running, cx)
        } else {
            Empty.into_any_element()
        };
        let mut card = Collapsible::new()
            .open(open)
            .min_w_0()
            .gap_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .child(header)
            .child(self.elapsed(format!("task-time-{id}"), Some(task.timing), running, cx))
            .child(div().text_sm().child(task.objective.clone()));
        if running
            && connected
            && let Some(activity) = task.activity()
        {
            card = card.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(activity.label())
                    .when_some(activity.detail(), |row, detail| row.child(detail)),
            );
        }
        if let Some(error) = task.failure() {
            card = card.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(super::commands::failure_text(error)),
            );
        }
        if let Some(reason) = &task.blocked_reason {
            let key = format!("panel.assistantBlockedReasons.{reason}");
            let label = crate::text::translate(&key);
            card = card.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(if label == key { reason.clone() } else { label }),
            );
        }
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(card.content(body))
            .when(
                !task.artifacts.is_empty() || !task.results.is_empty(),
                |card| {
                    card.child(super::resources::cards(
                        format!("task-{id}-artifacts"),
                        &cx.entity().downgrade(),
                        &task.artifacts,
                        &task.results,
                        self.resource_catalog.as_deref(),
                        cx,
                    ))
                },
            )
            .into_any_element()
    }

    pub(super) fn plan_card(
        &self,
        id: String,
        plan: &StatisticalPlan,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("plan-details-{id}");
        let open = self.expanded.get(&key).copied().unwrap_or(false);
        let mode = match plan.analysis_mode {
            AnalysisMode::Confirmatory => "confirmatory",
            AnalysisMode::Exploratory => "exploratory",
            AnalysisMode::PostHoc => "post_hoc",
        };
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .bg(cx.theme().muted)
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("panel.assistantPlan")),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(plan.research_question.clone()),
            )
            .child(plan_field("panel.assistantPlanMode", mode.to_owned(), cx))
            .child(plan_field(
                "panel.assistantPlanWorkflow",
                plan.selected_workflow.to_string(),
                cx,
            ))
            .child(div().text_sm().child(plan.study_design.description.clone()))
            .child(
                Button::new(SharedString::from(key.clone()))
                    .small()
                    .ghost()
                    .self_start()
                    .icon(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(crate::text::t("panel.details"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.expanded.insert(key.clone(), !open);
                        cx.notify();
                    })),
            )
            .when(open, |card| {
                card.child(TextView::markdown(
                    SharedString::from(id),
                    format!(
                        "```json\n{}\n```",
                        serde_json::to_string_pretty(plan)
                            .expect("validated statistical plan is JSON")
                    ),
                ))
            })
            .into_any_element()
    }
}

fn plan_field(label: &'static str, value: String, cx: &gpui::App) -> impl IntoElement {
    div()
        .min_w_0()
        .flex()
        .flex_wrap()
        .gap_2()
        .text_xs()
        .child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::t(label)),
        )
        .child(div().min_w_0().child(value))
}
