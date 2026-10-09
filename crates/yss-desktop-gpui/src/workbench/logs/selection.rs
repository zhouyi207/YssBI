//! Explicit row inspection, keyboard navigation and the existing workbench Details target.
use super::*;
use gpui::{ClipboardItem, KeyDownEvent, ScrollStrategy};

impl LogsPanel {
    pub(super) fn inspect(
        &mut self,
        entry: Rc<LogEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .visible
            .iter()
            .any(|index| self.entries[*index].key == entry.key)
        {
            return;
        }
        if self
            .selected
            .as_ref()
            .is_none_or(|selected| selected.read(cx).key() != &entry.key)
        {
            self.selected = Some(cx.new(|cx| LogDetails::new(entry, window, cx)));
        }
        window.focus(&self.focus, cx);
        cx.emit(LogsEvent::Inspect(self.selected.as_ref().unwrap().clone()));
        cx.notify();
    }

    pub(super) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(selected) = self.selected.take() {
            cx.emit(LogsEvent::Clear(selected.entity_id()));
            cx.notify();
        }
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
        if event.keystroke.key == "escape" {
            self.clear_selection(cx);
            cx.stop_propagation();
            return;
        }
        let current = self.selected.as_ref().and_then(|selected| {
            let key = selected.read(cx).key();
            self.visible
                .iter()
                .position(|index| &self.entries[*index].key == key)
        });
        let last = self.visible.len().checked_sub(1);
        let index = match event.keystroke.key.as_str() {
            "up" => current.map(|index| index.saturating_sub(1)).or(last),
            "down" => last.map(|last| current.map_or(0, |index| (index + 1).min(last))),
            "home" => last.map(|_| 0),
            "end" => last,
            "enter" => current.or(last.map(|_| 0)),
            _ => return,
        };
        if let Some(index) = index {
            let entry = self.entries[self.visible[index]].clone();
            self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
            self.inspect(entry, window, cx);
        }
        cx.stop_propagation();
    }
}

pub(super) fn copy_record(entry: &LogEntry, cx: &mut App) {
    if let Ok(text) = serde_json::to_string_pretty(&entry.record) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

impl super::super::Workbench {
    pub(in crate::workbench) fn connect_logs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subscriptions.push(cx.subscribe_in(
            &self.logs,
            window,
            |view, logs, event, window, cx| {
                match event {
                    LogsEvent::Inspect(log) if !view.is_closing(cx) => {
                        view.details
                            .update(cx, |details, cx| details.show_log(log, cx));
                        view.show_panel(super::super::menus::WorkbenchPanel::Details, window, cx);
                        // Revealing Details should leave row navigation with the log list.
                        window.focus(&logs.read(cx).focus_handle(cx), cx);
                    }
                    LogsEvent::Clear(id) => {
                        view.details
                            .update(cx, |details, cx| details.clear_log_if(*id, cx));
                    }
                    LogsEvent::Inspect(_) => {}
                }
            },
        ));
    }
}
