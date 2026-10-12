//! One selected immutable record, rendered with native read-only fields and text selection.
use super::*;
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    input::{Input, Textarea, TextareaState},
};
use gpui_kit::{ClipboardItem, Render, SharedString};
use yss_logging::LogOrigin;

struct LogSection {
    label: &'static str,
    text: Entity<TextareaState>,
    open: bool,
}

pub(in crate::workbench) struct LogDetails {
    entry: Rc<LogEntry>,
    fields: Vec<(&'static str, Entity<InputState>)>,
    sections: Vec<LogSection>,
    locale: &'static str,
}

impl LogDetails {
    pub(super) fn new(entry: Rc<LogEntry>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let record = &entry.record;
        let mut values = vec![
            ("detail.fields.time", record.timestamp.clone()),
            ("detail.fields.stream", record.stream_id.clone()),
            ("detail.fields.sequence", record.sequence.to_string()),
            (
                "detail.fields.level",
                entry::level_label(record.level).into(),
            ),
            (
                "detail.fields.type",
                crate::text::translate(entry::domain_key(Some(record.domain))),
            ),
            (
                "detail.fields.origin",
                match record.origin {
                    LogOrigin::Rust => "rust",
                    LogOrigin::Frontend => "frontend",
                }
                .into(),
            ),
            ("detail.fields.target", record.target.clone()),
        ];
        if let Some(event) = &record.event {
            values.push(("detail.fields.event", event.clone()));
        }
        if let Some(source) = &record.source {
            values.push(("detail.fields.source", source.clone()));
        }
        let fields = values
            .into_iter()
            .map(|(label, value)| {
                (
                    label,
                    cx.new(|cx| InputState::new(window, cx).default_value(value)),
                )
            })
            .collect();
        let mut content = vec![("detail.fields.message", record.message.clone())];
        if !record.fields.is_empty() {
            content.push((
                "detail.fields.fields",
                serde_json::to_string_pretty(&record.fields).unwrap_or_default(),
            ));
        }
        let sections = content
            .into_iter()
            .map(|(label, value)| LogSection {
                label,
                text: cx.new(|cx| TextareaState::new(window, cx).default_value(value).rows(6)),
                open: true,
            })
            .collect();
        Self {
            entry,
            fields,
            sections,
            locale: crate::text::locale(),
        }
    }

    pub(super) fn key(&self) -> &SharedString {
        &self.entry.key
    }

    fn section(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let section = &self.sections[index];
        let text = section.text.clone();
        Collapsible::new()
            .open(section.open)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(section.label)
                            .small()
                            .ghost()
                            .flex_1()
                            .min_w_0()
                            .label(crate::text::translate(section.label))
                            .icon(if section.open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.sections[index].open = !view.sections[index].open;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("copy-log-section", index))
                            .small()
                            .ghost()
                            .icon(IconName::Copy)
                            .tooltip(crate::text::translate("menubar.copy"))
                            .accessibility_label(crate::text::translate("menubar.copy"))
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    text.read(cx).value().to_string(),
                                ))
                            }),
                    ),
            )
            .content(
                div()
                    .px_3()
                    .py_2()
                    .font_family(cx.theme().mono_font_family.clone())
                    .child(
                        Textarea::new(&section.text)
                            .readonly(true)
                            .small()
                            .h(px(150.)),
                    ),
            )
    }
}

impl Render for LogDetails {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = crate::text::locale();
        if self.locale != locale {
            self.locale = locale;
            if let Some((_, value)) = self
                .fields
                .iter()
                .find(|(label, _)| *label == "detail.fields.type")
            {
                value.update(cx, |input, cx| {
                    input.set_value(
                        crate::text::translate(entry::domain_key(Some(self.entry.record.domain))),
                        window,
                        cx,
                    )
                });
            }
        }
        let entry = self.entry.clone();
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(crate::text::translate("log.title")),
                    )
                    .child(
                        Button::new("copy-log-record")
                            .small()
                            .ghost()
                            .icon(IconName::Copy)
                            .tooltip(crate::text::translate("native.logs.copyRecord"))
                            .accessibility_label(crate::text::translate("native.logs.copyRecord"))
                            .on_click(move |_, _, cx| super::selection::copy_record(&entry, cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .px_3()
                    .py_1()
                    .children(self.fields.iter().map(|(label, value)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .text_xs()
                            .child(
                                div()
                                    .w(gpui_kit::relative(0.35))
                                    .min_w_0()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(crate::text::translate(label)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child(
                                        Input::new(value).readonly(true).small().appearance(false),
                                    ),
                            )
                    })),
            )
            .children((0..self.sections.len()).map(|index| self.section(index, cx)))
    }
}
