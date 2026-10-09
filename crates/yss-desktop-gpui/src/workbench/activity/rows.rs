//! Visible indices change only with the document, expansion or conversation query.
use super::*;

impl ActivityPanel {
    pub(super) fn rebuild_rows(&mut self, cx: &App) {
        let query = self
            .search
            .as_ref()
            .map(|search| search.read(cx).value().to_lowercase())
            .unwrap_or_default();
        self.rows.clear();
        let mut hidden_depth = None;
        let Some(document) = &self.document else {
            return;
        };
        for (index, row) in document.rows.iter().enumerate() {
            if hidden_depth.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            hidden_depth = None;
            let visible = match &row.content {
                ActivityRowContent::Category {
                    default_expanded, ..
                } => {
                    if !self
                        .expanded
                        .get(&row.id)
                        .copied()
                        .unwrap_or(*default_expanded)
                    {
                        hidden_depth = Some(row.depth);
                    }
                    true
                }
                ActivityRowContent::Message { .. } => self.panel_id != "project",
                ActivityRowContent::Item(ActivityItem::Conversation {
                    title, session_id, ..
                }) => {
                    conversations::title(title).to_lowercase().contains(&query)
                        || session_id.to_lowercase().contains(&query)
                }
                ActivityRowContent::Item(
                    ActivityItem::Command { .. } | ActivityItem::Plugin { .. },
                ) => false,
                ActivityRowContent::Item(_) => true,
            };
            if visible {
                self.rows.push(index);
            }
        }
        if self.focused_row.as_ref().is_some_and(|id| {
            !self
                .rows
                .iter()
                .any(|index| &document.rows[*index].id == id)
        }) {
            self.focused_row = None;
        }
    }
}
