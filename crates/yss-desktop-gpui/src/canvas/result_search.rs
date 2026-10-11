//! Search shares the canvas catalogue and reads provenance only while open.
mod list;

use super::{CanvasEvent, GraphCanvas, ResultEntry};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    list::{List, ListState},
    popover::Popover,
};
use gpui_kit::{
    Anchor, AnyElement, App, Context, Entity, Focusable, IntoElement, Subscription, Task,
    WeakEntity, Window, div, prelude::*, rems,
};
use list::{Results, Source};
use std::{collections::BTreeMap, rc::Rc};
use yss_graph_execution::{plan::ResultCategory, result::ResultValidity};

struct Picker {
    open: bool,
    list: Entity<ListState<Results>>,
    queried: bool,
    locale: &'static str,
    task: Option<Task<()>>,
    _subscription: Subscription,
}

impl Picker {
    fn new(owner: &Entity<GraphCanvas>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.new(|cx| {
            ListState::new(
                Results {
                    owner: owner.downgrade(),
                    catalog: Rc::default(),
                    sources: BTreeMap::new(),
                    query: String::new(),
                    matches: vec![],
                    search_index: vec![],
                    selected: None,
                    loading: false,
                    failed: false,
                    enabled: false,
                },
                window,
                cx,
            )
            .searchable(true)
        });
        let subscription = cx.subscribe_in(owner, window, |picker, owner, event, window, cx| {
            if picker.open && matches!(event, CanvasEvent::InspectResult(_)) {
                picker.set_open(false, window, cx);
                owner.update(cx, |_, cx| cx.notify());
            }
        });
        Self {
            open: false,
            list,
            queried: false,
            locale: crate::text::locale(),
            task: None,
            _subscription: subscription,
        }
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open = open;
        if open {
            self.read_sources(window, cx);
        } else {
            self.task = None;
            self.queried = false;
            self.list.update(cx, |list, cx| {
                let delegate = list.delegate_mut();
                delegate.sources.clear();
                delegate.search_index.clear();
                delegate.loading = false;
                delegate.failed = false;
                list.set_query("", window, cx);
            });
        }
        cx.notify();
    }

    fn sync(
        &mut self,
        catalog: &Rc<[ResultEntry]>,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = !Rc::ptr_eq(&self.list.read(cx).delegate().catalog, catalog);
        let locale = crate::text::locale();
        if changed {
            self.task = None;
            self.queried = false;
        }
        self.list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            let enable_changed = delegate.enabled != enabled;
            delegate.enabled = enabled;
            if changed || locale != self.locale {
                delegate.catalog = catalog.clone();
                delegate.rebuild();
                let selected = (!delegate.matches.is_empty())
                    .then_some(gpui_kit::component::IndexPath::default());
                list.set_selected_index(selected, window, cx);
                cx.notify();
            } else if enable_changed {
                cx.notify();
            }
        });
        self.locale = locale;
        if self.open && !self.queried {
            self.queried = true;
            cx.defer_in(window, |picker, window, cx| {
                if picker.open && picker.task.is_none() {
                    picker.read_sources(window, cx);
                }
            });
        }
    }

    fn read_sources(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let delegate = self.list.read(cx).delegate();
        let Some(owner) = delegate.owner.upgrade() else {
            return;
        };
        let canvas = owner.read(cx);
        let catalog = delegate.catalog.clone();
        let project = canvas.graph.project.clone();
        let version = canvas.graph.editing.version;
        let path = canvas.graph.projection.graph_path.clone();
        let task = canvas.services.run(move |services| {
            let app = &services.application;
            app.current_graph_document(&project, &path, version)?;
            let sources = app
                .query_graph_results(&path)?
                .into_iter()
                .filter(|snapshot| snapshot.validity == ResultValidity::CurrentValid)
                .map(|snapshot| {
                    let result = snapshot.result;
                    let source = Source {
                        reference: result.provenance().reference(),
                        output: result.output().clone(),
                        run: result.provenance().run_id().get(),
                        plot: matches!(result.value().category(), ResultCategory::PlotData(_)),
                    };
                    (source.reference.result_id, source)
                })
                .collect::<BTreeMap<_, _>>();
            app.current_graph_document(&project, &path, version)?;
            Ok(sources)
        });
        self.queried = true;
        self.list.update(cx, |list, cx| {
            list.delegate_mut().loading = true;
            list.delegate_mut().failed = false;
            cx.notify();
        });
        self.task = Some(cx.spawn_in(window, async move |picker, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = picker.update_in(cx, |picker, window, cx| {
                if !picker.open || !Rc::ptr_eq(&picker.list.read(cx).delegate().catalog, &catalog) {
                    return;
                }
                picker.task = None;
                let refresh = picker.list.update(cx, |list, cx| {
                    let delegate = list.delegate_mut();
                    delegate.loading = false;
                    delegate.failed = result.is_err();
                    delegate.sources = result.unwrap_or_default();
                    let refresh = !delegate.failed
                        && catalog.iter().any(|entry| {
                            delegate
                                .sources
                                .get(&entry.reference.result_id)
                                .is_none_or(|source| {
                                    source.reference != entry.reference
                                        || source.output != entry.output
                                })
                        });
                    delegate.rebuild();
                    let selected = (!delegate.matches.is_empty())
                        .then_some(gpui_kit::component::IndexPath::default());
                    list.set_selected_index(selected, window, cx);
                    cx.notify();
                    refresh
                });
                let owner = picker.list.read(cx).delegate().owner.clone();
                let _ = owner.update(cx, |canvas, cx| {
                    if refresh {
                        canvas.refresh(cx);
                    } else {
                        cx.notify();
                    }
                });
                cx.notify();
            });
        }));
    }
}

impl GraphCanvas {
    pub(super) fn result_search(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity();
        let picker = window.use_keyed_state("pin-result-search", cx, |window, cx| {
            Picker::new(&owner, window, cx)
        });
        picker.update(cx, |picker, cx| {
            picker.sync(self.result_entries(), self.can_edit(), window, cx)
        });
        let state = picker.read(cx);
        let open = state.open;
        let list = state.list.clone();
        let control = picker.clone();
        let focus = list.focus_handle(cx);
        let owner = owner.downgrade();
        Popover::new("pin-result-search")
            .open(open)
            .anchor(Anchor::TopLeft)
            .track_focus(&focus)
            .trigger(
                Button::new("search-pin-results")
                    .small()
                    .ghost()
                    .icon(IconName::Search)
                    .disabled(!open && (!self.can_edit() || self.result_entries().is_empty()))
                    .tooltip(crate::text::t(if open {
                        "canvas.pinResultSearch.close"
                    } else {
                        "canvas.pinResultSearch.open"
                    }))
                    .accessibility_label(crate::text::t("canvas.pinResultSearch.open")),
            )
            .on_open_change(move |open, window, cx| {
                control.update(cx, |picker, cx| picker.set_open(*open, window, cx));
                let _ = owner.update(cx, |_, cx| cx.notify());
            })
            .content(move |_, _, cx| {
                let failed = list.read(cx).delegate().failed;
                let retry = picker.clone();
                div()
                    .w(rems(320. / 14.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(List::new(&list).max_h(rems(264. / 14.)).search_placeholder(
                        crate::text::t("canvas.pinResultSearch.searchPlaceholder"),
                    ))
                    .when(failed, |view| {
                        view.child(
                            Button::new("retry-result-search")
                                .small()
                                .ghost()
                                .label(crate::text::t("common.retry"))
                                .on_click(move |_, window, cx| {
                                    retry.update(cx, |picker, cx| picker.read_sources(window, cx))
                                }),
                        )
                    })
            })
            .into_any_element()
    }
}
