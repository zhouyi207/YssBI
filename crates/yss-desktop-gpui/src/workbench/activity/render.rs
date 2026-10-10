//! Render only requested visible rows; documents and actions remain with ActivityPanel.
use super::*;
use crate::text::activity_text;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, input::Input, tooltip::Tooltip};
use gpui_kit::{AnyElement, uniform_list};

impl Render for ActivityPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.search.is_some() && self.search_default_title != conversations::title("") {
            self.search_default_title = conversations::title("");
            self.rebuild_rows(cx);
        }
        if let Some(search) = &self.search {
            crate::text::input_placeholder(
                search,
                "native.workbench.searchConversations",
                window,
                cx,
            );
        }
        div()
            .id("activity-document")
            .track_focus(&self.focus)
            .role(gpui_kit::accesskit::Role::Tree)
            .aria_label(self.title_text())
            .on_key_down(cx.listener(Self::key_down))
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().sidebar)
            .px_1()
            .py_1()
            .when_some(self.search.as_ref(), |view, search| {
                view.child(
                    div().px_1().pb_1().child(
                        Input::new(search)
                            .small()
                            .prefix(Icon::new(IconName::Search).size_3()),
                    ),
                )
            })
            .children(self.tools(cx))
            .children(self.feedback(cx))
            .when(!self.rows.is_empty(), |body| {
                body.child(
                    uniform_list(
                        "activity-rows",
                        self.rows.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, window, cx| {
                            range
                                .filter_map(|index| view.render_row(view.rows[index], window, cx))
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .min_h_0()
                    .track_scroll(&self.scroll),
                )
            })
    }
}

impl ActivityPanel {
    fn render_row(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let document = self.document.as_ref()?;
        let row = document.rows.get(index)?;
        let expected_focus = document.clone();
        let focused = self
            .focused_row
            .as_ref()
            .map_or(self.rows.first() == Some(&index), |id| id == &row.id);
        let id = row.id.clone();
        let item = div()
            .id(gpui_kit::SharedString::from(id.clone()))
            .role(gpui_kit::accesskit::Role::TreeItem)
            .aria_label(row_label(&row.content))
            .aria_level(row.depth + 1)
            .when(focused, |row| row.aria_active_descendant())
            .when(
                focused && self.focus.is_focused(window) && window.last_input_was_keyboard(),
                |row| row.bg(cx.theme().muted),
            )
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(move |view, _, window, cx| {
                    if view.accepts(&expected_focus) {
                        view.focus_row(index, window, cx);
                    }
                }),
            )
            .pl(px(16. + row.depth as f32 * 16.))
            .pr_2()
            .h(px(if self.panel_id == "assistant" {
                52.
            } else {
                28.
            }))
            .w_full()
            .min_w_0()
            .rounded_sm()
            .flex()
            .items_center()
            .gap_1p5()
            .text_size(px(13.));
        let item = match &row.content {
            ActivityRowContent::Item(ActivityItem::Conversation { .. }) => {
                return Some(self.render_conversation(index, item, cx));
            }
            ActivityRowContent::Category { .. } => {
                return Some(self.render_category(index, item, cx));
            }
            ActivityRowContent::Item(ActivityItem::Node { .. }) => {
                return Some(self.render_node(index, item, cx));
            }
            ActivityRowContent::Item(
                ActivityItem::EventGraph { .. }
                | ActivityItem::FunctionGraph { .. }
                | ActivityItem::Doc { .. }
                | ActivityItem::Database { .. }
                | ActivityItem::Mind { .. }
                | ActivityItem::Chart { .. },
            ) => return Some(self.render_resource(index, item, cx)),
            ActivityRowContent::Message { label, description } => {
                let label = activity_text(label);
                let description = description.as_ref().map(activity_text);
                let hint = description.as_ref().map_or_else(
                    || label.clone(),
                    |description| format!("{label}\n{description}"),
                );
                item.text_color(cx.theme().muted_foreground)
                    .when_some(description, |row, description| {
                        row.aria_description(description)
                    })
                    .child(div().min_w_0().flex_1().truncate().child(label))
                    .tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx))
                    .into_any_element()
            }
            _ => return None,
        };
        Some(item)
    }
}

fn row_label(content: &ActivityRowContent) -> String {
    match content {
        ActivityRowContent::Category { label, .. }
        | ActivityRowContent::Message { label, .. }
        | ActivityRowContent::Item(ActivityItem::Command { label, .. }) => activity_text(label),
        ActivityRowContent::Item(ActivityItem::Conversation { title, .. }) if title.is_empty() => {
            conversations::title("").into_owned()
        }
        ActivityRowContent::Item(
            ActivityItem::Conversation { title, .. } | ActivityItem::Node { title, .. },
        ) => title.clone(),
        ActivityRowContent::Item(
            ActivityItem::EventGraph { name, .. }
            | ActivityItem::FunctionGraph { name, .. }
            | ActivityItem::Chart { name, .. }
            | ActivityItem::Mind { name, .. }
            | ActivityItem::Doc { name, .. }
            | ActivityItem::Database { name, .. }
            | ActivityItem::Plugin { name, .. },
        ) => name.clone(),
    }
}
