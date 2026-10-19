//! One selected immutable record, rendered with native read-only fields and text selection.
use super::*;
use gpui_kit::component::{
    ActiveTheme, Sizable,
    input::{Input, Textarea, TextareaState},
};
use gpui_kit::{ClipboardItem, Render, SharedString, relative};
use yss_ui::inspector::{self, InspectorHeader, InspectorSection};

struct LogSection {
    label: &'static str,
    text: Entity<TextareaState>,
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

    fn section(&self, index: usize, cx: &App) -> InspectorSection {
        let section = &self.sections[index];
        let text = section.text.clone();
        InspectorSection::new(
            ("log-section", index),
            crate::text::translate(section.label),
        )
        .tool(
            inspector::action_button(
                ("copy-log-section", index),
                IconName::Copy,
                crate::text::translate("menubar.copy"),
            )
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(text.read(cx).value().to_string()))
            }),
        )
        .child(
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .child(Textarea::new(&section.text).readonly(true).small().h_40()),
        )
    }
}

/// Record metadata keeps a fixed label lane so long, selectable values start on one edge.
fn field_row(label: &str, value: &Entity<InputState>, cx: &App) -> impl IntoElement + use<> {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .child(
            div()
                .w(relative(0.3))
                .flex_shrink_0()
                .truncate()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::translate(label)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(cx.theme().mono_font_family.clone())
                .child(Input::new(value).readonly(true).small().appearance(false)),
        )
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
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                InspectorHeader::new(
                    IconName::SquareTerminal,
                    crate::text::translate("log.title"),
                )
                .tool(
                    inspector::action_button(
                        "copy-log-record",
                        IconName::Copy,
                        crate::text::translate("native.logs.copyRecord"),
                    )
                    .on_click(move |_, _, cx| super::selection::copy_record(&entry, cx)),
                ),
            )
            .child(
                InspectorSection::new("log-fields", crate::text::translate("detail.sections.info"))
                    .child(inspector::property_list(
                        self.fields
                            .iter()
                            .map(|(label, value)| field_row(label, value, cx)),
                    )),
            )
            .children((0..self.sections.len()).map(|index| self.section(index, cx)))
    }
}
