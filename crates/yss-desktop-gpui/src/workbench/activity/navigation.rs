//! Keyboard focus is a row identity in the original document, independent of the open resource.
use super::*;
use gpui::{KeyDownEvent, ScrollStrategy};

impl ActivityPanel {
    pub(super) fn focused_index(&self) -> Option<usize> {
        let document = self.document.as_ref()?;
        self.rows
            .iter()
            .position(|index| self.focused_row.as_ref() == Some(&document.rows[*index].id))
            .or_else(|| (!self.rows.is_empty()).then_some(0))
    }

    pub(super) fn focus_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self
            .document
            .as_ref()
            .and_then(|document| document.rows.get(index))
        else {
            return;
        };
        self.focused_row = Some(row.id.clone());
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus.is_focused(window) || event.keystroke.modifiers != gpui::Modifiers::default()
        {
            return;
        }
        let Some(current) = self.focused_index() else {
            return;
        };
        let last = self.rows.len() - 1;
        let next = match event.keystroke.key.as_str() {
            "up" => current.saturating_sub(1),
            "down" => (current + 1).min(last),
            "home" => 0,
            "end" => last,
            "enter" | "space" => {
                self.activate_row(self.rows[current], cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        };
        self.focus_row(self.rows[next], window, cx);
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        cx.stop_propagation();
    }

    fn activate_row(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(document) = self.document.clone() else {
            return;
        };
        match &document.rows[index].content {
            ActivityRowContent::Category {
                default_expanded, ..
            } => {
                let row = &document.rows[index];
                let expanded = self
                    .expanded
                    .get(&row.id)
                    .copied()
                    .unwrap_or(*default_expanded);
                self.expanded.insert(row.id.clone(), !expanded);
                self.rebuild_rows(cx);
                cx.notify();
            }
            ActivityRowContent::Item(ActivityItem::Node { creation, .. }) => {
                cx.emit(ActivityEvent::InspectNode(
                    crate::catalog_rows::node_type(creation).clone(),
                ));
            }
            ActivityRowContent::Item(ActivityItem::Conversation { session_id, .. }) => {
                cx.emit(ActivityEvent::OpenConversation(session_id.clone()));
            }
            _ => self.open_resource(&document, index, cx),
        }
    }
}
