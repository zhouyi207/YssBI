//! Header presentation borrows the conversation's existing metadata and next-turn options.
use super::{ConversationEvent, ConversationPanel};
use gpui_kit::Context;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Selectable, Sizable,
    button::{Button, ButtonVariants},
};
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

    pub fn read_only_button(&self, cx: &mut Context<Self>) -> Button {
        let read_only = self.options.mode == HarnessMode::Ask;
        Button::new("conversation-read-only")
            .small()
            .ghost()
            .icon(if read_only {
                IconName::Lock
            } else {
                IconName::LockOpen
            })
            .selected(read_only)
            .label(yss_ui::text::t(if read_only {
                "panel.assistantModes.ask"
            } else {
                "panel.assistantModes.write"
            }))
            .accessibility_label(yss_ui::text::t(if read_only {
                "panel.assistantAllowEdits"
            } else {
                "panel.assistantReadOnly"
            }))
            .tooltip(yss_ui::text::t(if read_only {
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
