//! Timing and configuration are read from the recorded turn, never the current draft.
use super::{
    ConversationPanel,
    projection::{Timing, Turn, TurnState},
};
use gpui::{AnyElement, App, Context, SharedString, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme, tooltip::Tooltip};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use yss_harness_contract::{HarnessMode, ReasoningEffort};

impl ConversationPanel {
    pub(super) fn timing_connected(&self) -> bool {
        self.ready && !self.refreshing && self.stream_error.is_none() && self.transcript.running()
    }

    pub(super) fn refresh_timing(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.timing_connected() {
            return;
        }
        // Window-owned state disappears when this conversation stops rendering.
        // One timer refreshes all visible timings; the task holds only a weak entity.
        window.use_keyed_state("assistant-timing-clock", cx, |_, cx| {
            cx.spawn(async move |clock, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    if clock.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            })
        });
    }

    pub(super) fn execution_header(&self, turn: &Turn, cx: &App) -> AnyElement {
        let model_hint = format!("{} · {}", turn.model.provider_name, turn.model.model_name);
        div()
            .min_w_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .text_size(px(11.))
            .text_color(cx.theme().muted_foreground)
            .child(crate::text::t("panel.assistant"))
            .child(self.elapsed(
                format!("turn-time-{}", turn.id),
                Some(turn.timing),
                turn.state == TurnState::Running,
                cx,
            ))
            .child(
                div()
                    .id(SharedString::from(format!("turn-model-{}", turn.id)))
                    .max_w(px(208.))
                    .truncate()
                    .tooltip(move |window, cx| Tooltip::new(model_hint.clone()).build(window, cx))
                    .child(turn.model.model_name.clone()),
            )
            .when_some(turn.options, |header, options| {
                header.child(
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .gap_1()
                        .child(crate::text::t(match options.mode {
                            HarnessMode::Ask => "panel.assistantModes.ask",
                            HarnessMode::Write => "panel.assistantModes.write",
                        }))
                        .when_some(options.reasoning_effort, |group, effort| {
                            group.child(crate::text::t(match effort {
                                ReasoningEffort::Low => "panel.assistantEffort.low",
                                ReasoningEffort::Medium => "panel.assistantEffort.medium",
                                ReasoningEffort::High => "panel.assistantEffort.high",
                            }))
                        }),
                )
            })
            .into_any_element()
    }

    pub(super) fn elapsed(
        &self,
        id: String,
        timing: Option<Timing>,
        running: bool,
        cx: &App,
    ) -> AnyElement {
        let Some(timing) = timing else {
            return gpui::Empty.into_any_element();
        };
        let active = running && self.timing_connected();
        let value = if let Some(finished) = timing.finished_at {
            crate::text::format(
                "panel.assistantDuration",
                &[(
                    "value",
                    duration(finished.saturating_sub(timing.started_at)),
                )],
            )
        } else if active {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let elapsed = now
                .saturating_sub(u128::from(timing.started_at))
                .min(u128::from(u64::MAX)) as u64;
            crate::text::format("panel.assistantElapsed", &[("value", duration(elapsed))])
        } else {
            crate::text::t("panel.assistantTimingUnconfirmed").to_owned()
        };
        div()
            .id(SharedString::from(id))
            .text_size(px(11.))
            .text_color(cx.theme().muted_foreground)
            .tooltip(move |window, cx| Tooltip::new(timing_hint(timing)).build(window, cx))
            .child(value)
            .into_any_element()
    }
}

fn duration(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    if seconds < 60 {
        let tenths = milliseconds.saturating_add(50) / 100;
        let value = if tenths.is_multiple_of(10) {
            (tenths / 10).to_string()
        } else {
            format!("{}.{}", tenths / 10, tenths % 10)
        };
        crate::text::format("panel.assistantDurationSeconds", &[("value", value)])
    } else if seconds < 3600 {
        crate::text::format(
            "panel.assistantDurationMinutes",
            &[
                ("minutes", (seconds / 60).to_string()),
                ("seconds", (seconds % 60).to_string()),
            ],
        )
    } else {
        crate::text::format(
            "panel.assistantDurationHours",
            &[
                ("hours", (seconds / 3600).to_string()),
                ("minutes", ((seconds % 3600) / 60).to_string()),
                ("seconds", (seconds % 60).to_string()),
            ],
        )
    }
}

pub(super) fn timestamp(at: u64) -> String {
    i64::try_from(at)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| at.to_string())
}

fn timing_hint(timing: Timing) -> String {
    let start = crate::text::format(
        "panel.assistantStartedAt",
        &[("value", timestamp(timing.started_at))],
    );
    let end = crate::text::format(
        if timing.finished_at.is_some() {
            "panel.assistantFinishedAt"
        } else {
            "panel.assistantUpdatedAt"
        },
        &[(
            "value",
            timestamp(timing.finished_at.unwrap_or(timing.updated_at)),
        )],
    );
    format!("{start}\n{end}")
}
