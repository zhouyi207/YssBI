//! Header presentation borrows the conversation's existing metadata and next-turn options.
use super::{ConversationEvent, ConversationPanel};
use gpui_kit::Context;
use yss_harness_contract::HarnessMode;

impl ConversationPanel {
    pub fn is_refreshing(&self) -> bool {
        self.refreshing
    }

    pub fn display_title(&self) -> String {
        self.session
            .conversation
            .as_ref()
            .map(|metadata| metadata.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| yss_ui::text::t("panel.assistantNewConversation").into())
    }

    pub(super) fn set_mode(&mut self, mode: HarnessMode, cx: &mut Context<Self>) {
        if self.options.mode != mode {
            self.options.mode = mode;
            cx.emit(ConversationEvent::OptionsChanged);
            cx.notify();
        }
    }
}
