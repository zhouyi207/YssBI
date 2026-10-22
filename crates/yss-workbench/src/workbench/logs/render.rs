use super::*;
use gpui_kit::component::{
    ActiveTheme,
    menu::{ContextMenuExt, PopupMenuItem},
    tooltip::Tooltip,
};
use gpui_kit::{Render, uniform_list};
use yss_ui::tool_panel::ToolPanel;

impl Render for LogsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.following() {
            self.scroll.scroll_to_bottom();
        }
        let locale = crate::text::locale();
        if self.view_locale != locale {
            self.view_locale = locale;
            self.search.update(cx, |input, cx| {
                input.set_placeholder(crate::text::translate("log.searchPlaceholder"), window, cx)
            });
        }
        let body = if self.visible.is_empty() {
            let (title, hint) = if self.entries.is_empty() {
                ("log.noLogs", "native.workbench.logsEmpty")
            } else {
                ("log.noMatches", "log.adjustFilterHint")
            };
            crate::appearance::empty_state(
                IconName::SquareTerminal,
                crate::text::translate(title),
                crate::text::translate(hint),
                cx,
            )
        } else {
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
                            log_row(entry.clone(), selected.as_ref() == Some(&entry.key), cx)
                        })
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .map(crate::appearance::row_gutter)
            .into_any_element()
        };
        div()
            .id("logs")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|view, event, window, cx| view.key_down(event, window, cx)))
            .size_full()
            .child(
                ToolPanel::new(crate::text::translate("log.title"), body).tools(self.toolbar(cx)),
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
    let row = div()
        .id(entry.key.clone())
        .role(gpui_kit::accesskit::Role::ListItem)
        .aria_label(entry.preview.clone())
        .aria_selected(selected)
        .cursor_pointer()
        .w_full()
        .rounded_sm();
    yss_ui::appearance::row_outline(row, selected, false, cx)
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
                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                            message.to_string(),
                        ))
                    }),
            )
            .item(
                PopupMenuItem::new(crate::text::translate("native.logs.copyRecord"))
                    .icon(IconName::Copy)
                    .on_click(move |_, _, cx| super::selection::copy_record(&record, cx)),
            )
        })
}
