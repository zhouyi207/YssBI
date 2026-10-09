//! Indices into the Application projection; no copied node or category model.
use super::*;
use std::collections::BTreeSet;
use yss_application::activity_panel::ActivityRow;

pub(super) struct Browser {
    pub catalog: Option<Arc<ActivityPanelDocument>>,
    pub collapsed: BTreeSet<String>,
    pub query: Box<str>,
    pub visible: Vec<usize>,
    selectable: Vec<usize>,
    pub active: Option<usize>,
    pub generation: u64,
}

impl Browser {
    pub fn new(catalog: Option<Arc<ActivityPanelDocument>>) -> Self {
        let mut browser = Self {
            catalog,
            collapsed: BTreeSet::new(),
            query: "".into(),
            visible: vec![],
            selectable: vec![],
            active: None,
            generation: 0,
        };
        browser.rebuild();
        browser
    }

    pub fn install(&mut self, catalog: Arc<ActivityPanelDocument>) {
        self.collapsed.retain(|id| {
            catalog.rows.iter().any(|row| {
                &row.id == id && matches!(row.content, ActivityRowContent::Category { .. })
            })
        });
        self.catalog = Some(catalog);
        self.rebuild();
    }

    pub fn clear_catalog(&mut self) {
        self.catalog = None;
        self.rebuild();
    }

    pub fn set_query(&mut self, query: &str) {
        if query != self.query.as_ref() {
            self.query = query.into();
            self.rebuild();
        }
    }

    pub fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    pub fn row(&self, visible_index: usize) -> Option<&ActivityRow> {
        self.catalog
            .as_ref()?
            .rows
            .get(*self.visible.get(visible_index)?)
    }

    pub fn toggle(&mut self, index: usize) {
        if self.searching() {
            return;
        }
        let Some(row) = self.row(index) else {
            return;
        };
        if !matches!(row.content, ActivityRowContent::Category { .. }) {
            return;
        }
        let id = row.id.clone();
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.rebuild();
    }

    pub fn toggle_all(&mut self) {
        if self.searching() {
            return;
        }
        if self.collapsed.is_empty() {
            self.collapsed = self
                .catalog
                .iter()
                .flat_map(|catalog| &catalog.rows)
                .filter(|row| matches!(row.content, ActivityRowContent::Category { .. }))
                .map(|row| row.id.clone())
                .collect();
        } else {
            self.collapsed.clear();
        }
        self.rebuild();
    }

    pub fn move_selection(&mut self, forward: bool) {
        let Some(last) = self.selectable.len().checked_sub(1) else {
            return;
        };
        let current = self
            .active
            .and_then(|active| self.selectable.iter().position(|index| *index == active));
        let next = if forward {
            current.map_or(0, |index| (index + 1).min(last))
        } else {
            current.map_or(last, |index| index.saturating_sub(1))
        };
        self.active = Some(self.selectable[next]);
    }

    fn rebuild(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.visible.clear();
        self.selectable.clear();
        self.active = None;
        let Some(catalog) = &self.catalog else {
            return;
        };
        let query = yss_node_catalog::normalize_catalog_search_text(&self.query);
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let searching = self.searching();
        let mut included = BTreeSet::new();
        let mut ancestors: Vec<usize> = vec![];
        for (index, row) in catalog.rows.iter().enumerate() {
            while ancestors
                .last()
                .is_some_and(|index| catalog.rows[*index].depth >= row.depth)
            {
                ancestors.pop();
            }
            match &row.content {
                ActivityRowContent::Category { .. } => ancestors.push(index),
                ActivityRowContent::Item(ActivityItem::Node { search_text, .. })
                    if terms.iter().all(|term| search_text.contains(term)) =>
                {
                    included.insert(index);
                    included.extend(ancestors.iter().copied());
                }
                _ => {}
            }
        }
        let mut hidden_depth = None;
        for index in included {
            let row = &catalog.rows[index];
            if hidden_depth.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            hidden_depth = None;
            let visible_index = self.visible.len();
            self.visible.push(index);
            match &row.content {
                ActivityRowContent::Category { .. }
                    if !searching && self.collapsed.contains(&row.id) =>
                {
                    hidden_depth = Some(row.depth)
                }
                ActivityRowContent::Item(ActivityItem::Node {
                    available: true, ..
                }) => self.selectable.push(visible_index),
                _ => {}
            }
        }
        self.active = self.selectable.first().copied();
    }
}
