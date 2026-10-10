//! Conversation visibility comes from DockArea; the closed ID is only a reopening hint.
use super::{ConversationPanel, Workbench};
use gpui_kit::component::dock::DockPlacement;
use gpui_kit::{App, Context, Entity, Focusable, Window};
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};

impl Workbench {
    fn docked_conversations<'a>(
        &'a self,
        cx: &'a App,
    ) -> impl Iterator<Item = Entity<ConversationPanel>> + 'a {
        let dock = self.dock.read(cx);
        [
            DockPlacement::Center,
            DockPlacement::Left,
            DockPlacement::Right,
            DockPlacement::Bottom,
        ]
        .into_iter()
        .filter_map(move |placement| dock.layout(placement))
        .flat_map(|tree| tree.panels())
        .filter_map(|id| dock.panel(id)?.view().downcast::<ConversationPanel>().ok())
    }

    pub(in crate::workbench) fn conversation_window_open(&self, cx: &App) -> bool {
        self.docked_conversations(cx).next().is_some()
    }

    pub(in crate::workbench) fn toggle_conversation_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || self.assistant_reopen || self.dock.read(cx).is_locked() {
            return;
        }
        if self.project.is_none() {
            self.show_assistant(window, cx);
        } else if self.conversation_window_open(cx) {
            self.close_conversation_window(window, cx);
        } else {
            self.assistant_reopen = true;
            // A read started before this click must not choose a now-obsolete target.
            self.invalidate_assistant_directory(window, cx);
        }
    }

    pub(in crate::workbench) fn close_conversation_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || self.dock.read(cx).is_locked() {
            return;
        }
        let panels: Vec<_> = self.docked_conversations(cx).collect();
        if panels.is_empty() {
            return;
        }
        self.assistant_reopen = false;
        let selected = self
            .visible_conversation(cx)
            .or_else(|| panels.first().cloned());
        if let Some(selected) = selected {
            self.assistant_closed = Some(selected.read(cx).session.id.to_string());
        }
        self.dock.update(cx, |dock, cx| {
            dock.set_zoomed_out(window, cx);
            for panel in panels {
                dock.remove_panel(panel, window, cx);
            }
            dock.focus_handle(cx).focus(window, cx);
        });
        self.sync_active_conversation(cx);
        cx.notify();
    }

    pub(super) fn resume_conversation(
        &mut self,
        directory: &ActivityPanelDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.assistant_reopen {
            return;
        }
        if self.is_closing(cx) {
            self.assistant_reopen = false;
            return;
        }
        let mut sessions = directory.rows.iter().filter_map(|row| {
            if let ActivityRowContent::Item(ActivityItem::Conversation { session_id, .. }) =
                &row.content
            {
                Some(session_id)
            } else {
                None
            }
        });
        let recent = sessions.next();
        let previous = self.assistant_closed.as_ref().and_then(|previous| {
            recent
                .into_iter()
                .chain(sessions)
                .find(|id| *id == previous)
        });
        if let Some(id) = previous.or(recent) {
            self.open_conversation(id.clone(), window, cx);
        } else {
            self.new_conversation(window, cx);
        }
    }
}
