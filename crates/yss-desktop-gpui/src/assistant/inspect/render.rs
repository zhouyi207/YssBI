//! Tool details use localized facts and the shared resource cards, with raw JSON on demand.
use super::{CopyTarget, Inspection, Loading};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    spinner::Spinner,
    text::TextView,
};
use gpui_kit::{Context, IntoElement, Render, SharedString, Window, div, prelude::*, px};
use serde_json::Value;

impl Render for Inspection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = &self.card.tool;
        let title = crate::assistant::activity::tool_name(tool.kind);
        let failed = tool.state.failed();
        let active = tool.running() && self.card.connected;
        let mut row = div()
            .min_w_0()
            .flex_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2();
        row = if active {
            row.child(Spinner::new().small().icon(IconName::LoaderCircle))
        } else {
            row.child(
                Icon::new(if tool.running() {
                    IconName::Clock
                } else if failed {
                    IconName::CircleAlert
                } else {
                    IconName::Check
                })
                .small(),
            )
        };
        row = row
            .child(div().min_w_0().flex_1().truncate().child(title.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(if failed {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(tool.state.label(self.card.connected)),
            )
            .child(crate::assistant::execution::elapsed(
                "tool-time".into(),
                self.timing(),
                active,
                cx,
            ));
        let header = Button::new("toggle-tool")
            .small()
            .ghost()
            .w_full()
            .h_auto()
            .icon(if self.open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .accessibility_label(title.clone())
            .tooltip(title)
            .child(row)
            .on_click(cx.listener(|view, _, window, cx| {
                view.open = !view.open;
                if view.open && view.loading == Loading::Pending {
                    view.load(window, cx);
                }
                cx.notify();
            }));
        let mut card = Collapsible::new()
            .open(self.open)
            .min_w_0()
            .gap_1()
            .child(header);
        if let Some(target) = self
            .detail
            .as_ref()
            .and_then(|detail| detail.target.as_ref())
        {
            card = card.child(
                div()
                    .pl_6()
                    .min_w_0()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(target.clone()),
            );
        }
        let failure = tool.failure.as_deref().or_else(|| {
            self.detail
                .as_ref()
                .and_then(|detail| detail.failure.as_ref())
                .and_then(|failure| failure.get("code")?.as_str())
        });
        if let Some(code) = failure {
            card = card.child(
                div()
                    .px_2()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(crate::assistant::commands::failure_text(code)),
            );
        }
        if self.open {
            card = card.content(self.details(cx));
        }
        card
    }
}

impl Inspection {
    fn details(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .text_xs();
        if self.loading == Loading::Reading {
            body = body.child(crate::text::t("panel.assistantLoadingDetails"));
        }
        if self.loading == Loading::Failed {
            body = body
                .child(
                    div()
                        .text_color(cx.theme().danger)
                        .child(crate::text::t("panel.assistantDetailsFailed")),
                )
                .child(
                    Button::new("retry-tool")
                        .small()
                        .ghost()
                        .self_start()
                        .label(crate::text::t("panel.assistantRetryDetails"))
                        .on_click(cx.listener(|view, _, window, cx| view.load(window, cx))),
                );
        }
        let Some(detail) = &self.detail else {
            return body;
        };
        if let Some(details) = detail
            .failure
            .as_ref()
            .and_then(|failure| failure.get("details"))
            .and_then(Value::as_object)
        {
            let mut failure = div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .text_color(cx.theme().danger);
            for (key, value) in details {
                failure = failure.child(fact(
                    key,
                    value,
                    matches!(key.as_str(), "reason" | "category"),
                    cx,
                ));
            }
            body = body.child(failure);
        }
        for (key, value) in &detail.parameters {
            body = body.child(fact(key, value, true, cx));
        }
        body = body.child(crate::assistant::resources::cards(
            "tool-artifacts",
            &self.card.owner,
            &detail.artifacts,
            &detail.results,
            self.card.catalog.as_deref(),
            cx,
        ));
        if let Some(resource) = self.target.clone() {
            body = body.child(
                Button::new("open-tool-target")
                    .small()
                    .ghost()
                    .self_start()
                    .icon(IconName::FileText)
                    .label(crate::text::t("panel.assistantOpenResource"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let _ = view
                            .card
                            .owner
                            .update(cx, |owner, cx| owner.open_reference(&resource, cx));
                    })),
            );
        }
        body = body.child(
            Button::new("tool-technical")
                .small()
                .ghost()
                .self_start()
                .icon(if self.technical_open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .label(crate::text::t("panel.assistantTechnicalDetails"))
                .on_click(cx.listener(|view, _, _, cx| {
                    view.technical_open = !view.technical_open;
                    cx.notify();
                })),
        );
        if self.technical_open {
            let json = self
                .technical
                .get_or_insert_with(|| {
                    SharedString::from(format!(
                        "```json\n{}\n```",
                        serde_json::to_string_pretty(detail)
                            .expect("tool inspection contains JSON values")
                    ))
                })
                .clone();
            body = body
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(self.copy_button(
                            "copy-tool-arguments",
                            CopyTarget::Arguments,
                            "panel.assistantToolArguments",
                            cx,
                        ))
                        .child(self.copy_button(
                            "copy-tool-detail",
                            CopyTarget::Details,
                            "menubar.copy",
                            cx,
                        )),
                )
                .child(
                    div()
                        .id("tool-json")
                        .min_w_0()
                        .max_h(px(280.))
                        .overflow_y_scroll()
                        .child(TextView::markdown("tool-inspection", json)),
                );
            if self.copied.is_some() {
                body = body.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::t("panel.assistantCopied")),
                );
            }
        }
        body
    }

    fn copy_button(
        &self,
        id: &'static str,
        target: CopyTarget,
        label: &'static str,
        cx: &Context<Self>,
    ) -> Button {
        Button::new(id)
            .small()
            .ghost()
            .icon(if self.copied == Some(target) {
                IconName::Check
            } else {
                IconName::Copy
            })
            .label(crate::text::t(label))
            .on_click(cx.listener(move |view, _, _, cx| view.copy(target, cx)))
    }
}

fn fact(key: &str, value: &Value, translate_value: bool, cx: &gpui_kit::App) -> impl IntoElement {
    let label = translated("panel.assistantToolFacts", key);
    let value = match value.as_str() {
        Some(value) if translate_value => translated("panel.assistantToolValues", value),
        Some(value) => value.to_owned(),
        None => value.to_string(),
    };
    div()
        .min_w_0()
        .flex()
        .flex_wrap()
        .gap_2()
        .child(div().text_color(cx.theme().muted_foreground).child(label))
        .child(div().min_w_0().child(value))
}

fn translated(prefix: &str, value: &str) -> String {
    let key = format!("{prefix}.{value}");
    let label = crate::text::translate(&key);
    if label == key {
        value.to_owned()
    } else {
        label
    }
}
