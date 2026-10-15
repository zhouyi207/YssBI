//! Conversation rows use the original directory identity and the workbench's mutation state.
use super::*;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{ContextMenuExt, PopupMenuItem},
    tooltip::Tooltip,
};
use gpui_kit::{AnyElement, Div, Stateful};
use yss_ui::lifecycle::LogReleased;
use yss_ui::sidebar;

pub(super) fn title(value: &str) -> std::borrow::Cow<'_, str> {
    if value.is_empty() {
        crate::text::t("panel.assistantNewConversation")
    } else {
        value.into()
    }
}

impl ActivityPanel {
    pub(super) fn conversation_busy(&self, cx: &App) -> bool {
        self.conversation_owner.as_ref().is_some_and(|owner| {
            owner
                .upgrade()
                .is_none_or(|owner| owner.read(cx).is_closing(cx))
        })
    }

    pub(super) fn render_conversation(
        &self,
        index: usize,
        item: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(document) = self.document.as_ref() else {
            return item.into_any_element();
        };
        let ActivityRowContent::Item(ActivityItem::Conversation {
            session_id,
            title: name,
            last_opened_at,
        }) = &document.rows[index].content
        else {
            return item.into_any_element();
        };
        let label = title(name).into_owned();
        let time = i64::try_from(*last_opened_at)
            .ok()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|time| time.naive_utc().format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| crate::text::translate("native.assistant.timeUnavailable"));
        let hint = format!("{label}\n{time}");
        let expected_open = document.clone();
        let open_id = session_id.clone();
        let expected_rename = document.clone();
        let expected_menu = document.clone();
        let owner = cx.entity().downgrade();
        item.group("activity-conversation")
            .aria_description(time.clone())
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(sidebar::item_title(label).flex_1())
                    .child(
                        Button::new(format!("conversation-rename:{session_id}"))
                            .ghost()
                            .small()
                            .size_5()
                            .flex_shrink_0()
                            .icon(IconName::Pencil)
                            .tooltip(crate::text::translate("panel.assistantRenameConversation"))
                            .disabled(self.conversation_busy(cx))
                            .opacity(0.)
                            .group_hover("activity-conversation", |button| button.opacity(1.))
                            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation()
                            })
                            .on_click(cx.listener(move |view, _, _, cx| {
                                cx.stop_propagation();
                                view.rename_row(&expected_rename, index, cx);
                            })),
                    ),
            )
            .child(sidebar::item_detail(time, cx))
            .tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx))
            .on_click(cx.listener(move |view, _, _, cx| {
                if view.accepts(&expected_open) {
                    cx.emit(ActivityEvent::ActivateConversation(open_id.clone()));
                }
            }))
            .context_menu(move |menu, _, cx| {
                let busy = owner
                    .upgrade()
                    .is_none_or(|owner| owner.read(cx).conversation_busy(cx));
                let rename = owner.clone();
                let rename_document = expected_menu.clone();
                let delete = owner.clone();
                let delete_document = expected_menu.clone();
                menu.item(
                    PopupMenuItem::new(crate::text::translate("panel.assistantRenameConversation"))
                        .icon(IconName::Pencil)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            rename
                                .update(cx, |view, cx| view.rename_row(&rename_document, index, cx))
                                .log_released();
                        }),
                )
                .separator()
                .item(
                    PopupMenuItem::new(crate::text::translate(
                        "native.workbench.deleteConversation",
                    ))
                    .icon(IconName::Trash)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        delete
                            .update(cx, |view, cx| {
                                if view.accepts(&delete_document)
                                    && let ActivityRowContent::Item(ActivityItem::Conversation {
                                        session_id,
                                        title,
                                        ..
                                    }) = &delete_document.rows[index].content
                                {
                                    cx.emit(ActivityEvent::DeleteConversation(
                                        session_id.clone(),
                                        title.clone(),
                                    ));
                                }
                            })
                            .log_released();
                    }),
                )
            })
            .into_any_element()
    }

    fn rename_row(
        &self,
        expected: &Arc<ActivityPanelDocument>,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.accepts(expected)
            && !self.conversation_busy(cx)
            && let ActivityRowContent::Item(ActivityItem::Conversation {
                session_id, title, ..
            }) = &expected.rows[index].content
        {
            cx.emit(ActivityEvent::RenameConversation(
                session_id.clone(),
                title.clone(),
            ));
        }
    }
}
