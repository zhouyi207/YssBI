//! Render only requested visible rows; documents and actions remain with ActivityPanel.
use super::*;
use crate::text::activity_text;
use gpui::{AnyElement, uniform_list};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    input::Input,
    menu::{ContextMenuExt, PopupMenuItem},
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;

impl Render for ActivityPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .role(gpui::accesskit::Role::Tree)
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
            .id(gpui::SharedString::from(id.clone()))
            .role(gpui::accesskit::Role::TreeItem)
            .aria_label(row_label(&row.content))
            .aria_level(row.depth + 1)
            .when(focused, |row| row.aria_active_descendant())
            .border_1()
            .border_color(if focused && self.focus.is_focused(window) {
                cx.theme().ring
            } else {
                gpui::hsla(0., 0., 0., 0.)
            })
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |view, _, window, cx| {
                    if view.accepts(&expected_focus) {
                        view.focus_row(index, window, cx);
                    }
                }),
            )
            .pl(px(16. + row.depth as f32 * 16.))
            .pr_2()
            .h(px(28.))
            .w_full()
            .min_w_0()
            .rounded_sm()
            .flex()
            .items_center()
            .gap_1p5()
            .text_size(px(13.));
        let item = match &row.content {
            ActivityRowContent::Item(ActivityItem::Conversation {
                session_id, title, ..
            }) => {
                let active = self.active_resource.as_deref() == Some(session_id.as_str());
                let open_id = session_id.clone();
                let rename_id = session_id.clone();
                let title = title.clone();
                let expected = document.clone();
                let owner = cx.entity().downgrade();
                item.child(Icon::new(IconName::MessageSquareText).size_3())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(if title.is_empty() {
                                crate::text::translate("panel.assistantNewConversation")
                            } else {
                                title.clone()
                            }),
                    )
                    .cursor_pointer()
                    .when(active, |view| view.bg(cx.theme().sidebar_accent))
                    .hover(|view| view.bg(cx.theme().muted))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(ActivityEvent::OpenConversation(open_id.clone()))
                    }))
                    .context_menu(move |menu, _, _| {
                        let owner = owner.clone();
                        let expected = expected.clone();
                        let id = rename_id.clone();
                        let title = title.clone();
                        menu.item(
                            PopupMenuItem::new(crate::text::translate(
                                "contextMenu.dialog.renameSubmit",
                            ))
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |view, cx| {
                                    if view.accepts(&expected) {
                                        cx.emit(ActivityEvent::RenameConversation(
                                            id.clone(),
                                            title.clone(),
                                        ));
                                    }
                                });
                            }),
                        )
                    })
                    .into_any_element()
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
            crate::text::translate("panel.assistantNewConversation")
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
