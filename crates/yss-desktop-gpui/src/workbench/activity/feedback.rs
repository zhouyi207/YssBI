//! Query feedback projects the existing workbench request; it does not start another query.
use super::*;
use gpui_kit::AnyElement;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    spinner::Spinner,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::workbench) enum ReadState {
    Loading,
    Ready,
    Failed(&'static str),
}

impl ActivityPanel {
    pub(in crate::workbench) fn set_read_state(
        &mut self,
        state: ReadState,
        cx: &mut Context<Self>,
    ) {
        if self.read_state != state {
            self.read_state = state;
            cx.notify();
        }
    }

    pub(super) fn title_text(&self) -> String {
        self.document.as_ref().map_or_else(
            || crate::text::translate("panel.assistantConversations"),
            |document| activity_text(&document.title),
        )
    }

    pub(super) fn tools(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let document = self.document.as_ref()?;
        if document.tools.is_empty() {
            return None;
        }
        Some(
            div()
                .h_7()
                .px_2()
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .children(document.tools.iter().map(|tool| {
                    let expected = document.clone();
                    let id = tool.id.to_owned();
                    Button::new(gpui_kit::SharedString::from(format!("activity-tool-{id}")))
                        .ghost()
                        .small()
                        .size_5()
                        .icon(tool_icon(tool.icon))
                        .disabled(self.conversation_busy(cx))
                        .tooltip(activity_text(&tool.label))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.accepts(&expected) {
                                cx.emit(ActivityEvent::Tool(id.clone()));
                            }
                        }))
                }))
                .into_any_element(),
        )
    }

    pub(super) fn feedback(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        match self.read_state {
            ReadState::Loading if self.document.is_none() => Some(
                div()
                    .id("activity-loading")
                    .role(gpui_kit::accesskit::Role::Status)
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(Spinner::new().small().icon(IconName::LoaderCircle))
                    .child(crate::text::translate("common.loading"))
                    .into_any_element(),
            ),
            ReadState::Failed(key) => Some(
                div()
                    .id("activity-error")
                    .role(gpui_kit::accesskit::Role::Alert)
                    .px_3()
                    .py_2()
                    .flex()
                    .items_start()
                    .gap_1()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(div().min_w_0().flex_1().child(crate::text::translate(key)))
                    .child(
                        Button::new("activity-retry")
                            .ghost()
                            .small()
                            .size_5()
                            .icon(IconName::RefreshCw)
                            .tooltip(crate::text::translate("common.retry"))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(ActivityEvent::RefreshResources)),
                            ),
                    )
                    .into_any_element(),
            ),
            _ if self.rows.is_empty() => {
                let document = self.document.as_ref()?;
                let (title, description) = document.empty_state.as_ref()?;
                let filtered = self
                    .search
                    .as_ref()
                    .is_some_and(|search| !search.read(cx).value().is_empty());
                Some(
                    div()
                        .id("activity-empty")
                        .role(gpui_kit::accesskit::Role::Status)
                        .px_3()
                        .py_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .text_xs()
                        .child(activity_text(title))
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(if filtered {
                                    crate::text::translate("panel.assistantNoMatchingConversations")
                                } else {
                                    activity_text(description)
                                }),
                        )
                        .into_any_element(),
                )
            }
            _ => None,
        }
    }
}

pub(super) fn tool_icon(icon: &str) -> IconName {
    match icon {
        "install" => IconName::Package,
        "refresh" => IconName::RefreshCw,
        _ => IconName::Plus,
    }
}

impl super::super::Workbench {
    pub(in crate::workbench) fn activity_read_state(
        &self,
        panels: &[&str],
        state: ReadState,
        cx: &mut Context<Self>,
    ) {
        for id in panels {
            if let Some(panel) = self
                .activities
                .get(id)
                .and_then(gpui_kit::WeakEntity::upgrade)
            {
                panel.update(cx, |panel, cx| panel.set_read_state(state, cx));
            }
        }
    }
}
