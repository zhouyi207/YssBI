//! List owns input, navigation and focus; the delegate holds read projections and row indices.
use super::{RecentProjectEvent, RecentProjects, RecentSnapshot};
use gpui::{App, Context, Entity, IntoElement, Task, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    list::{ListDelegate, ListItem, ListState},
};

pub(crate) struct RecentDelegate {
    owner: Entity<RecentProjects>,
    snapshot: RecentSnapshot,
    query: String,
    matches: Vec<usize>,
    selected: Option<IndexPath>,
}

impl RecentDelegate {
    pub(crate) fn new(owner: Entity<RecentProjects>, snapshot: RecentSnapshot) -> Self {
        let mut delegate = Self {
            owner,
            snapshot,
            query: String::new(),
            matches: vec![],
            selected: None,
        };
        delegate.filter();
        delegate
    }

    pub(crate) fn install(&mut self, snapshot: RecentSnapshot) -> Option<IndexPath> {
        self.snapshot = snapshot;
        self.filter();
        (!self.matches.is_empty() && !self.snapshot.loading && self.snapshot.error.is_none())
            .then_some(IndexPath::default())
    }

    fn filter(&mut self) {
        self.matches = self
            .snapshot
            .records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                let name = record.name.to_lowercase();
                let path = record.path.to_lowercase();
                self.query
                    .split_whitespace()
                    .all(|part| name.contains(part) || path.contains(part))
                    .then_some(index)
            })
            .collect();
        self.selected = None;
    }
}

impl ListDelegate for RecentDelegate {
    type Item = ListItem;

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_lowercase();
        self.filter();
        cx.notify();
        Task::ready(())
    }

    fn items_count(&self, _: usize, _: &App) -> usize {
        // Keep the accepted records for a retry, but show the error even when they are nonempty.
        if self.snapshot.error.is_some() {
            0
        } else {
            self.matches.len()
        }
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let record = self.snapshot.records.get(*self.matches.get(index.row)?)?;
        Some(
            ListItem::new(gpui::SharedString::from(record.id.as_str().to_owned()))
                .h(px(56.))
                .px_3()
                .accessibility_label(format!("{} {}", record.name, record.path))
                .disabled(self.snapshot.loading || self.snapshot.error.is_some())
                .child(
                    Icon::new(IconName::FolderOpen)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_sm().truncate().child(record.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(cx.theme().muted_foreground)
                                .child(record.path.clone()),
                        ),
                ),
        )
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
    }

    fn confirm(&mut self, _: bool, _: &mut Window, cx: &mut Context<ListState<Self>>) {
        if self.snapshot.loading || self.snapshot.error.is_some() {
            return;
        }
        let Some(record) = self
            .selected
            .and_then(|index| self.matches.get(index.row))
            .and_then(|index| self.snapshot.records.get(*index))
            .cloned()
        else {
            return;
        };
        let generation = self.snapshot.generation;
        let picker = cx.entity_id();
        self.owner.update(cx, |_, cx| {
            cx.emit(RecentProjectEvent::Open {
                record,
                generation,
                picker,
            })
        });
    }

    fn cancel(&mut self, _: &mut Window, cx: &mut Context<ListState<Self>>) {
        let picker = cx.entity_id();
        self.owner
            .update(cx, |_, cx| cx.emit(RecentProjectEvent::Dismiss(picker)));
    }

    fn loading(&self, _: &App) -> bool {
        self.snapshot.loading
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .items_center()
            .justify_center()
            .p_4()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(crate::text::translate(
                self.snapshot
                    .error
                    .unwrap_or("native.projects.noMatchingRecentProjects"),
            ))
            .when(self.snapshot.error.is_some(), |view| {
                let owner = self.owner.clone();
                view.child(
                    Button::new("recent-projects-retry")
                        .small()
                        .ghost()
                        .label(crate::text::translate("common.retry"))
                        .on_click(move |_, _, cx| owner.update(cx, |recent, cx| recent.reload(cx))),
                )
            })
    }
}
