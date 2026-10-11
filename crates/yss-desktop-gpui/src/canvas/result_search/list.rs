//! A virtual list filters current outputs; selecting resolves the port again.
use super::*;
use gpui_kit::component::{
    IndexPath,
    list::{ListDelegate, ListItem},
};
use yss_graph_execution::{
    plan::PlanOutputRef,
    result::{ResultId, ResultReference},
};

pub(super) struct Source {
    pub reference: ResultReference,
    pub output: PlanOutputRef,
    pub run: u64,
    pub plot: bool,
}

pub(super) struct Results {
    pub owner: WeakEntity<GraphCanvas>,
    pub catalog: Rc<[ResultEntry]>,
    pub sources: BTreeMap<ResultId, Source>,
    pub query: String,
    pub matches: Vec<usize>,
    pub search_index: Vec<(usize, String)>,
    pub selected: Option<IndexPath>,
    pub loading: bool,
    pub failed: bool,
    pub enabled: bool,
}

impl Results {
    fn source(&self, entry: &ResultEntry) -> Option<&Source> {
        self.sources
            .get(&entry.reference.result_id)
            .filter(|source| source.reference == entry.reference && source.output == entry.output)
    }

    pub fn rebuild(&mut self) {
        self.search_index = self
            .catalog
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                if !entry.current() {
                    return None;
                }
                let source = self.source(entry)?;
                let search = format!(
                    "{} {} {} {}",
                    entry.search_label,
                    crate::results::window_title(source.plot),
                    entry.output.graph(),
                    source.run
                )
                .to_lowercase();
                Some((index, search))
            })
            .collect::<Vec<_>>();
        self.search_index.sort_by(|a, b| a.1.cmp(&b.1));
        self.filter();
    }

    fn filter(&mut self) {
        let query = self.query.trim().to_lowercase();
        self.matches = self
            .search_index
            .iter()
            .filter_map(|(index, search)| search.contains(&query).then_some(*index))
            .collect();
    }
}

impl ListDelegate for Results {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.matches.len()
    }
    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_owned();
        self.filter();
        cx.defer_in(window, |list, window, cx| {
            if list.selected_index().is_none() && !list.delegate().matches.is_empty() {
                list.set_selected_index(Some(IndexPath::default()), window, cx);
            }
            cx.notify();
        });
        Task::ready(())
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let entry = self.catalog.get(*self.matches.get(ix.row)?)?;
        let title = crate::results::window_title(self.source(entry)?.plot);
        Some(
            ListItem::new(("pin-result", ix.row))
                .h_8()
                .disabled(!self.enabled || self.loading || self.failed)
                .child(
                    div()
                        .id("pin-result-label")
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .truncate()
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(title.clone())
                                .build(window, cx)
                        })
                        .child(entry.title.clone()),
                )
                .text_color(cx.theme().foreground),
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
        if !self.enabled || self.loading || self.failed {
            return;
        }
        let Some(entry) = self
            .selected
            .and_then(|ix| self.matches.get(ix.row))
            .and_then(|index| self.catalog.get(*index))
        else {
            return;
        };
        let Some(address) = &entry.address else {
            return;
        };
        let _ = self.owner.update(cx, |canvas, cx| {
            if canvas.can_edit()
                && Rc::ptr_eq(canvas.result_entries(), &self.catalog)
                && entry.current()
            {
                canvas.inspect_port_result(address, false, cx);
            }
        });
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .p_3()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(crate::text::t(if self.failed {
                "native.results.unavailable"
            } else if self.query.trim().is_empty() {
                "canvas.pinResultSearch.empty"
            } else {
                "canvas.pinResultSearch.noMatches"
            }))
    }
}
