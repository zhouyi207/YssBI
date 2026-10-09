//! Header presentation borrows the conversation's existing metadata and next-turn options.
use super::{ConversationEvent, ConversationPanel};
use gpui::Context;
use gpui_component::{
    Selectable, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit_assets::IconName;
use yss_harness_contract::HarnessMode;

impl ConversationPanel {
    pub(crate) fn is_refreshing(&self) -> bool {
        self.refreshing
    }

    pub(crate) fn display_title(&self) -> String {
        self.session
            .conversation
            .as_ref()
            .map(|metadata| metadata.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| crate::text::t("panel.assistantNewConversation").into())
    }

    pub(crate) fn read_only_button(&self, cx: &mut Context<Self>) -> Button {
        let read_only = self.options.mode == HarnessMode::Ask;
        Button::new("conversation-read-only")
            .xsmall()
            .ghost()
            .icon(if read_only {
                IconName::Lock
            } else {
                IconName::LockOpen
            })
            .selected(read_only)
            .accessibility_label(crate::text::t("panel.assistantReadOnly"))
            .tooltip(crate::text::t(if read_only {
                "panel.assistantModeHint.ask"
            } else {
                "panel.assistantModeHint.write"
            }))
            .on_click(cx.listener(|view, _, _, cx| {
                view.set_mode(
                    if view.options.mode == HarnessMode::Ask {
                        HarnessMode::Write
                    } else {
                        HarnessMode::Ask
                    },
                    cx,
                );
            }))
    }

    pub(super) fn set_mode(&mut self, mode: HarnessMode, cx: &mut Context<Self>) {
        if self.options.mode != mode {
            self.options.mode = mode;
            cx.emit(ConversationEvent::OptionsChanged);
            cx.notify();
        }
    }
}
