//! Filtering keeps row indices over the existing bounded records; appends scan only new rows.
use super::*;

impl LogsPanel {
    pub(super) fn matches(&self, entry: &LogEntry) -> bool {
        self.levels.contains(&entry.record.level)
            && self
                .domain
                .is_none_or(|domain| domain == entry.record.domain)
            && entry.matches(&self.query)
    }

    pub(super) fn following(&self) -> bool {
        let scroll = self.scroll.0.borrow();
        // Match the reference viewport's tolerance for partial rows and resize rounding.
        self.auto_scroll
            && (scroll
                .deferred_scroll_to_item
                .is_some_and(|target| target.item_index == usize::MAX)
                || scroll.base_handle.max_offset().y + scroll.base_handle.offset().y < px(80.))
    }

    pub(super) fn refilter(&mut self) {
        let following = self.following();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| self.matches(entry).then_some(index))
            .collect();
        if following {
            self.scroll.scroll_to_bottom();
        }
    }

    pub(super) fn trim(&mut self, following: bool) {
        let removed = self.entries.len().saturating_sub(VIEW_CAPACITY);
        if removed == 0 {
            return;
        }
        self.truncated = true;
        let removed_visible = self.visible.partition_point(|index| *index < removed);
        self.entries.drain(..removed);
        self.visible.drain(..removed_visible);
        self.visible.iter_mut().for_each(|index| *index -= removed);
        if following {
            // The pending bottom scroll handles eviction; shifting the old offset would
            // make another batch arriving before layout mistake it for manual scrolling.
            return;
        }
        // Keep the same rows under the pointer when a bounded prefix is evicted.
        let state = self.scroll.0.borrow();
        let offset = state.base_handle.offset();
        state.base_handle.set_offset(gpui::point(
            offset.x,
            (offset.y + px(ROW_HEIGHT * removed_visible as f32)).min(px(0.)),
        ));
    }

    pub(super) fn clear_display(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        self.visible.clear();
        self.truncated = false;
        self.scroll
            .0
            .borrow()
            .base_handle
            .set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }
}
