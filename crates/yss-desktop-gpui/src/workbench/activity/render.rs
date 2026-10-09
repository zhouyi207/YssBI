//! Render only requested visible rows; documents and actions remain with ActivityPanel.
use super::*;
use crate::text::activity_text;
use gpui::{AnyElement, uniform_list};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{ContextMenuExt, PopupMenuItem},
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
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().sidebar)
            .px_1()
            .py_1()
            .when_some(self.search.as_ref(), |view, search| {
                view.child(div().px_1().pb_1().child(Input::new(search).small()))
            })
            .children(self.document.tools.iter().map(|tool| {
                let id = tool.id.to_owned();
                Button::new(gpui::SharedString::from(format!("activity-tool-{id}")))
                    .small()
                    .ghost()
                    .label(activity_text(&tool.label))
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(ActivityEvent::Tool(id.clone()))),
                    )
            }))
            .when(self.document.panel_id == "project", |view| {
                view.child(
                    Button::new("activity-import")
                        .small()
                        .ghost()
                        .icon(IconName::Database)
                        .label(crate::text::translate("menubar.importData"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ActivityEvent::ImportData))),
                )
            })
            .when(!self.rows.is_empty(), |body| {
                body.child(
                    uniform_list(
                        "activity-rows",
                        self.rows.len(),
                        cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                            range
                                .filter_map(|index| view.render_row(view.rows[index], cx))
                                .collect::<Vec<_>>()
                        }),
                    )
                    .flex_1()
                    .min_h_0()
                    .track_scroll(&self.scroll),
                )
            })
            .when(self.document.rows.is_empty(), |view| {
                view.when_some(self.document.empty_state.as_ref(), |view, (_, message)| {
                    view.child(
                        div()
                            .p_3()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(activity_text(message)),
                    )
                })
            })
    }
}

impl ActivityPanel {
    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let row = self.document.rows.get(index)?;
        let id = row.id.clone();
        let item = div()
            .id(gpui::SharedString::from(id.clone()))
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
        let item =
            match &row.content {
                ActivityRowContent::Item(ActivityItem::Conversation {
                    session_id, title, ..
                }) => {
                    let active = self.active_resource.as_deref() == Some(session_id.as_str());
                    let open_id = session_id.clone();
                    let rename_id = session_id.clone();
                    let title = title.clone();
                    let expected = self.document.clone();
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
                                        if Arc::ptr_eq(&view.document, &expected) {
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
                ActivityRowContent::Category {
                    label,
                    count,
                    default_expanded,
                    ..
                } => {
                    let expanded = self.expanded.get(&id).copied().unwrap_or(*default_expanded);
                    let expected = self.document.clone();
                    item.font_weight(gpui::FontWeight::MEDIUM)
                        .cursor_pointer()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|style| style.bg(cx.theme().muted))
                        .child(crate::catalog_rows::category(
                            activity_text(label),
                            expanded,
                            cx,
                        ))
                        .children(count.map(|count| {
                            div().text_xs().px_1().rounded_sm().child(count.to_string())
                        }))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if Arc::ptr_eq(&view.document, &expected) {
                                view.expanded.insert(id.clone(), !expanded);
                                view.rebuild_rows(cx);
                                cx.notify();
                            }
                        }))
                        .into_any_element()
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
                ActivityRowContent::Message { label, .. } => item
                    .text_color(cx.theme().muted_foreground)
                    .child(activity_text(label))
                    .into_any_element(),
                _ => return None,
            };
        Some(item)
    }
}
