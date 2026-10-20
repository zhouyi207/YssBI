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
use gpui_kit::{Context, IntoElement, Render, SharedString, Window, div, prelude::*, rems};
use serde_json::Value;
use yss_ui::lifecycle::LogReleased;

impl Render for Inspection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = &self.card.tool;
        let title = crate::assistant::activity::tool_name(&tool.kind);
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
            row.child(Icon::new(tool.state.icon(self.card.connected)).small())
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
        if let Some(detail) = &self.detail {
            let mut preview = div()
                .min_w_0()
                .pl_6()
                .pr_2()
                .flex()
                .flex_col()
                .gap_1()
                .text_xs();
            if let Some(target) = &detail.target {
                preview = preview.child(div().min_w_0().truncate().child(target.clone()));
            }
            let parameters = detail
                .parameters
                .iter()
                .filter(|(_, value)| !value.is_null());
            let has_meaningful = parameters
                .clone()
                .any(|(key, _)| !matches!(key.as_str(), "offset" | "limit"));
            let parameters = parameters
                .filter(|(key, _)| !has_meaningful || !matches!(key.as_str(), "offset" | "limit"))
                .take(2);
            for (key, value) in parameters {
                preview = preview.child(preview_fact(key, value, cx));
            }
            if let Some(output) = detail.output.as_ref().filter(|value| !value.is_null()) {
                preview = preview.child(
                    div()
                        .min_w_0()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::format(
                            "panel.assistantToolResultPreview",
                            &[("preview", output_preview(output))],
                        )),
                );
            }
            if !detail.artifacts.is_empty() || !detail.results.is_empty() {
                preview = preview.child(div().text_color(cx.theme().muted_foreground).child(
                    yss_ui::text::format(
                        "panel.assistantToolResources",
                        &[
                            (
                                "changes",
                                yss_ui::text::plural(
                                    "counts.resourceChanges",
                                    detail.artifacts.len(),
                                ),
                            ),
                            (
                                "results",
                                yss_ui::text::plural(
                                    "counts.computationResults",
                                    detail.results.len(),
                                ),
                            ),
                        ],
                    ),
                ));
            }
            card = card.child(preview);
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
            .px_2()
            .pb_2()
            .text_xs();
        if self.loading == Loading::Reading {
            body = body.child(yss_ui::text::t("panel.assistantLoadingDetails"));
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
                    .label(yss_ui::text::t("panel.assistantOpenResource"))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.card
                            .owner
                            .update(cx, |owner, cx| owner.open_reference(&resource, cx))
                            .log_released();
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
                .label(yss_ui::text::t("panel.assistantTechnicalDetails"))
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
                        .max_h(rems(17.5))
                        .overflow_y_scroll()
                        .child(TextView::markdown("tool-inspection", json)),
                );
            if self.copied.is_some() {
                body = body.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::t("panel.assistantCopied")),
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
            .label(yss_ui::text::t(label))
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
    yss_ui::text::optional(&format!("{prefix}.{value}")).unwrap_or_else(|| value.to_owned())
}

fn preview_fact(key: &str, value: &Value, cx: &gpui_kit::App) -> impl IntoElement {
    div()
        .min_w_0()
        .flex()
        .flex_wrap()
        .gap_2()
        .text_color(cx.theme().muted_foreground)
        .child(translated("panel.assistantToolFacts", key))
        .child(div().min_w_0().child(value_preview(value)))
}

fn output_preview(value: &Value) -> String {
    match value {
        Value::Object(fields) if !fields.is_empty() => {
            use std::fmt::Write;
            let mut preview = String::new();
            for (key, value) in fields.iter().filter(|(_, value)| !value.is_null()).take(3) {
                if !preview.is_empty() {
                    preview.push_str(" · ");
                }
                write!(
                    preview,
                    "{}: {}",
                    translated("panel.assistantToolFacts", key),
                    value_preview(value)
                )
                .expect("writing a string cannot fail");
            }
            if preview.is_empty() {
                value_preview(value)
            } else {
                preview
            }
        }
        value => value_preview(value),
    }
}

fn value_preview(value: &Value) -> String {
    match value {
        Value::String(value) => {
            let mut characters = value.chars();
            let mut preview: String = characters
                .by_ref()
                .take(120)
                .map(|character| {
                    if character.is_whitespace() {
                        ' '
                    } else {
                        character
                    }
                })
                .collect();
            if characters.next().is_some() {
                preview.push('…');
            }
            preview
        }
        Value::Array(items) => yss_ui::text::plural("counts.items", items.len()),
        Value::Object(fields) => yss_ui::text::plural("counts.fields", fields.len()),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::value_preview;
    use serde_json::Value;

    #[test]
    fn string_preview_is_bounded_without_changing_authored_data() {
        let source = format!("{}\n{}", "分析".repeat(60), "后续内容");
        let value = Value::String(source.clone());
        assert_eq!(value_preview(&value), format!("{}…", "分析".repeat(60)));
        assert_eq!(value.as_str(), Some(source.as_str()));
    }

    #[test]
    fn string_preview_flattens_lines_without_truncating_short_values() {
        assert_eq!(
            value_preview(&Value::String("first\nsecond".into())),
            "first second"
        );
        assert_eq!(
            value_preview(&Value::String("x".repeat(120))),
            "x".repeat(120)
        );
    }
}
