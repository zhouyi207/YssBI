//! Only visible rows are built; the toolbar and count share the same filtered indices.
use super::*;
use gpui::{Render, uniform_list};
use gpui_component::{
    ActiveTheme,
    menu::{ContextMenuExt, PopupMenuItem},
    tooltip::Tooltip,
};

impl Render for LogsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.following() {
            self.scroll.scroll_to_bottom();
        }
        let status_color = if self.error.is_some() {
            cx.theme().danger
        } else if self.connecting || self.truncated {
            cx.theme().warning
        } else {
            cx.theme().success
        };
        let locale = crate::text::locale();
        if self.view_locale != locale {
            self.view_locale = locale;
            self.search.update(cx, |input, cx| {
                input.set_placeholder(crate::text::translate("log.searchPlaceholder"), window, cx)
            });
        }
        div()
            .id("logs")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|view, event, window, cx| view.key_down(event, window, cx)))
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(self.toolbar(cx))
            .child(
                div()
                    .id("log-status")
                    .role(gpui::accesskit::Role::Status)
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        div()
                            .size(px(6.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(status_color),
                    )
                    .child(crate::text::format(
                        "log.showCount",
                        &[
                            ("filtered", self.visible.len().to_string()),
                            ("total", self.entries.len().to_string()),
                        ],
                    ))
                    .when(self.truncated && self.error.is_none(), |bar| {
                        bar.child(
                            div()
                                .text_color(cx.theme().warning)
                                .child(crate::text::translate("log.streamTruncated")),
                        )
                    })
                    .when(self.connecting, |bar| {
                        bar.child(crate::text::translate("log.loadingLogs"))
                    })
                    .when_some(self.error, |bar, key| {
                        bar.child(
                            div()
                                .text_color(cx.theme().danger)
                                .child(crate::text::translate(key)),
                        )
                    }),
            )
            .when(!self.visible.is_empty(), |view| {
                view.child(
                    uniform_list(
                        "logs-list",
                        self.visible.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                            let selected = view
                                .selected
                                .as_ref()
                                .map(|selected| selected.read(cx).key().clone());
                            range
                                .filter_map(|index| {
                                    view.visible
                                        .get(index)
                                        .and_then(|index| view.entries.get(*index))
                                })
                                .map(|entry| {
                                    log_row(
                                        entry.clone(),
                                        selected.as_ref() == Some(&entry.key),
                                        cx,
                                    )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&self.scroll)
                    .flex_1()
                    .min_h_0(),
                )
            })
            .when(
                self.visible.is_empty() && !self.connecting && self.error.is_none(),
                |view| {
                    let (title, hint) = if self.entries.is_empty() {
                        ("log.noLogs", "native.workbench.logsEmpty")
                    } else {
                        ("log.noMatches", "log.adjustFilterHint")
                    };
                    view.child(crate::appearance::empty_state(
                        IconName::SquareTerminal,
                        crate::text::translate(title),
                        crate::text::translate(hint),
                        cx,
                    ))
                },
            )
    }
}

fn log_row(
    entry: Rc<LogEntry>,
    selected: bool,
    cx: &mut Context<LogsPanel>,
) -> impl IntoElement + use<> {
    let record = &entry.record;
    let message = entry.message.clone();
    let inspected = entry.clone();
    let copied = entry.clone();
    let color = match record.level {
        LogLevel::Error => cx.theme().danger,
        LogLevel::Warn => cx.theme().warning,
        _ => cx.theme().foreground,
    };
    div()
        .id(entry.key.clone())
        .role(gpui::accesskit::Role::ListItem)
        .aria_label(entry.preview.clone())
        .aria_selected(selected)
        .cursor_pointer()
        .when(selected, |row| row.bg(cx.theme().accent))
        .hover(|row| row.bg(cx.theme().muted))
        .on_click(
            cx.listener(move |view, _, window, cx| view.inspect(inspected.clone(), window, cx)),
        )
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .font_family(cx.theme().mono_font_family.clone())
        .text_xs()
        .overflow_hidden()
        .child(
            div()
                .w(px(92.))
                .flex_shrink_0()
                .text_color(cx.theme().muted_foreground)
                .child(entry.time.clone()),
        )
        .child(
            div()
                .w(px(45.))
                .flex_shrink_0()
                .text_color(color)
                .child(entry::level_label(record.level)),
        )
        .child(
            div()
                .max_w(px(100.))
                .flex_shrink_0()
                .truncate()
                .text_color(cx.theme().muted_foreground)
                .child(crate::text::translate(entry::domain_key(Some(
                    record.domain,
                )))),
        )
        .when_some(record.source.as_ref(), |row, source| {
            row.child(
                div()
                    .max_w(px(100.))
                    .flex_shrink_0()
                    .truncate()
                    .text_color(cx.theme().accent_foreground)
                    .child(source.clone()),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(entry.preview.clone()),
        )
        .tooltip(move |window, cx| Tooltip::new(message.clone()).build(window, cx))
        .context_menu(move |menu, _, _| {
            let message = copied.message.clone();
            let record = copied.clone();
            menu.item(
                PopupMenuItem::new(crate::text::translate("native.logs.copyMessage"))
                    .icon(IconName::Copy)
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(message.to_string()))
                    }),
            )
            .item(
                PopupMenuItem::new(crate::text::translate("native.logs.copyRecord"))
                    .icon(IconName::Copy)
                    .on_click(move |_, _, cx| super::selection::copy_record(&record, cx)),
            )
        })
}
