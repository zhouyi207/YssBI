//! The composer reads turn usage without scanning or cloning its output history.
use super::{ConversationPanel, projection::usage::TurnUsage};
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    popover::Popover,
    progress::ProgressCircle,
    tooltip::Tooltip,
};
use gpui_kit::{Anchor, AnyElement, App, Context, Empty, div, prelude::*, px};

const FIELDS: [&str; 5] = [
    "panel.assistantUsageFields.inputTokens",
    "panel.assistantUsageFields.outputTokens",
    "panel.assistantUsageFields.cachedInputTokens",
    "panel.assistantUsageFields.cacheCreationInputTokens",
    "panel.assistantUsageFields.reasoningTokens",
];

impl ConversationPanel {
    pub(super) fn usage_indicator(&self, cx: &mut Context<Self>) -> AnyElement {
        let context = self
            .transcript
            .turns
            .last()
            .and_then(|turn| turn.consumption.latest)
            .and_then(|context| Some((context.input?, context.capacity?)));
        let percent = context.map(|(input, capacity)| {
            (u128::from(input) * 100 + u128::from(capacity) / 2) / u128::from(capacity)
        });
        let label = percent.map_or_else(
            || crate::text::t("panel.assistantTokens").to_owned(),
            |value| {
                crate::text::format(
                    "panel.assistantContextPercent",
                    &[("value", value.to_string())],
                )
            },
        );
        let owner = cx.entity().downgrade();
        Popover::new("assistant-token-usage")
            .anchor(Anchor::BottomRight)
            .trigger(
                Button::new("assistant-token-usage-trigger")
                    .xsmall()
                    .ghost()
                    .size_6()
                    .accessibility_label(label.clone())
                    .tooltip(label.clone())
                    .child(
                        ProgressCircle::new("assistant-context-usage")
                            .value(context.map_or(0., |(input, capacity)| {
                                (100. * input as f64 / f64::from(capacity)).min(100.) as f32
                            }))
                            .color(
                                if context.is_some_and(|(input, capacity)| {
                                    u128::from(input) * 100 >= u128::from(capacity) * 90
                                }) {
                                    cx.theme().warning
                                } else {
                                    cx.theme().primary
                                },
                            )
                            .accessibility_label(label),
                    ),
            )
            .content(move |_, _, cx| {
                owner.upgrade().map_or_else(
                    || Empty.into_any_element(),
                    |owner| owner.read(cx).usage_details(cx),
                )
            })
            .into_any_element()
    }

    fn usage_details(&self, cx: &App) -> AnyElement {
        let turn = self.transcript.turns.last();
        let empty = TurnUsage::default();
        let usage = turn.map_or(&empty, |turn| &turn.consumption);
        let input = usage
            .latest
            .and_then(|context| context.input)
            .map(u128::from);
        let capacity = usage
            .latest
            .and_then(|context| context.capacity)
            .map(u128::from);
        let mut content = div()
            .w(px(288.))
            .flex()
            .flex_col()
            .gap_2()
            .text_xs()
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(crate::text::t("panel.assistantTokens")),
            )
            .when_some(turn, |content, turn| {
                let model = format!("{} · {}", turn.model.provider_name, turn.model.model_name);
                let hint = model.clone();
                content.child(
                    div()
                        .id("assistant-usage-model")
                        .truncate()
                        .text_color(cx.theme().muted_foreground)
                        .tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx))
                        .child(model),
                )
            })
            .child(row(
                "panel.assistantLatestInput",
                format!("{} / {}", count(input), count(capacity)),
            ))
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_2()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(crate::text::format(
                        "panel.assistantTurnUsage",
                        &[("count", count(Some(u128::from(usage.calls))))],
                    )),
            );
        for (key, value) in FIELDS.into_iter().zip(usage.total) {
            content = content.child(row(key, count(value)));
        }
        content
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("panel.assistantUsageHint")),
            )
            .when(usage.incomplete, |content| {
                content.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t("panel.assistantUsagePartial")),
                )
            })
            .into_any_element()
    }
}

fn row(key: &str, value: String) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_wrap()
        .items_start()
        .gap_x_3()
        .gap_y_1()
        .child(div().flex_shrink_0().child(crate::text::translate(key)))
        .child(div().flex_1().max_w_full().text_right().child(value))
        .into_any_element()
}

fn count(value: Option<u128>) -> String {
    let Some(value) = value else {
        return crate::text::t("panel.assistantUsageUnknown").to_owned();
    };
    // Both supported locales use comma-separated groups of three for integers.
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}
