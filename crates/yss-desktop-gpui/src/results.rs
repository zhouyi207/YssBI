//! Native result panels own view state and one Application result lease.
mod query;
mod table;
mod value;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Window, div, prelude::*, px, uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    table::{DataTable, TableState},
};
use std::{collections::BTreeSet, sync::Arc};
use yss_application::graph::results::report::ResultTablePart;
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use crate::assets::NativeIcon;
use crate::services::NativeServices;
use query::{ResultContent, ResultLease};
use table::ResultGrid;

pub enum ResultEvent {
    Loaded(bool),
    Activated,
    Closed,
}

pub struct ResultPanel {
    services: Arc<NativeServices>,
    reference: ResultReference,
    lease: Option<ResultLease>,
    focus: FocusHandle,
    value: Option<Arc<RuntimeValue>>,
    rows: Vec<value::ValueRow>,
    expanded: BTreeSet<String>,
    tables: bool,
    table: Option<Entity<TableState<ResultGrid>>>,
    part: Option<ResultTablePart>,
    loading: bool,
    error: Option<String>,
    generation: u64,
    task: Option<gpui::Task<()>>,
    closed: bool,
}

impl ResultPanel {
    pub fn new(
        services: Arc<NativeServices>,
        reference: ResultReference,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            services,
            reference,
            lease: None,
            focus: cx.focus_handle(),
            value: None,
            rows: vec![],
            expanded: BTreeSet::new(),
            tables: false,
            table: None,
            part: None,
            loading: false,
            error: None,
            generation: 0,
            task: None,
            closed: false,
        }
    }
    pub fn available(&self) -> bool {
        !self.closed
    }
    pub fn loaded(&self) -> bool {
        self.lease.is_some()
    }

    pub fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.closed {
            return;
        }
        self.loading = true;
        self.error = None;
        self.generation += 1;
        let generation = self.generation;
        let reference = self.reference;
        let owner = self.services.clone();
        let task = self.services.run(move |_| query::open(owner, reference));
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.closed || view.generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok((lease, content)) => {
                        view.lease = Some(lease);
                        match content {
                            ResultContent::Value { value, tables } => {
                                view.tables = tables;
                                view.rows = value::rows(&value, &view.expanded, tables);
                                view.value = Some(value);
                            }
                            ResultContent::Page(page) => view.install_page(page, window, cx),
                        }
                    }
                    Err(_) => {
                        view.error = Some("结果不可用或读取失败，请检查当前项目和日志。".into())
                    }
                }
                cx.emit(ResultEvent::Loaded(view.loaded()));
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn install_page(&mut self, page: ResultGrid, window: &mut Window, cx: &mut Context<Self>) {
        self.table = Some(cx.new(|cx| TableState::new(page, window, cx).sortable(false)));
    }

    fn load_page(
        &mut self,
        part: Option<ResultTablePart>,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.closed || self.lease.is_none() {
            return;
        }
        self.loading = true;
        self.error = None;
        self.generation += 1;
        let generation = self.generation;
        let reference = self.reference;
        let owner = self.services.clone();
        let query_part = part.clone();
        let task = self
            .services
            .run(move |_| query::page(&owner, reference, query_part, offset));
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.closed || view.generation != generation {
                    return;
                }
                view.loading = false;
                match result {
                    Ok(page) => {
                        view.part = part;
                        view.install_page(page, window, cx);
                    }
                    Err(_) => view.error = Some("数据页读取失败，原结果保留，请重试。".into()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let row = &self.rows[index];
        let path = row.path.clone();
        let part = row.table.clone();
        div()
            .h(px(32.))
            .flex()
            .items_center()
            .gap_2()
            .pl(px(row.depth as f32 * 16. + 8.))
            .border_b_1()
            .border_color(cx.theme().border)
            .text_sm()
            .child(
                Button::new(("value-row", index))
                    .small()
                    .ghost()
                    .label(row.label.clone())
                    .disabled(!row.expandable && part.is_none())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if let Some(part) = &part {
                            view.load_page(Some(part.clone()), 0, window, cx);
                        } else {
                            if !view.expanded.insert(path.clone()) {
                                view.expanded.remove(&path);
                            }
                            if let Some(value) = &view.value {
                                view.rows = value::rows(value, &view.expanded, view.tables);
                            }
                            cx.notify();
                        }
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(if row.table.is_some() {
                        "查看数据".into()
                    } else {
                        row.value.clone()
                    }),
            )
    }
}

impl Render for ResultPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let paging = self.table.as_ref().map(|table| {
            let page = table.read(cx).delegate();
            (
                page.offset,
                page.row_count(),
                page.total_count,
                page.has_more,
            )
        });
        div()
            .id("result-view")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(
                div()
                    .p_2()
                    .flex()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .when(self.value.is_some(), |bar| {
                        bar.child(
                            Button::new("result-overview")
                                .small()
                                .ghost()
                                .icon(IconName::ArrowLeft)
                                .label("返回概览")
                                .disabled(self.loading)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.table = None;
                                    view.part = None;
                                    cx.notify();
                                })),
                        )
                    })
                    .when_some(paging, |bar, (offset, count, total, has_more)| {
                        bar.child(
                            Button::new("result-prev")
                                .small()
                                .ghost()
                                .label("上一页")
                                .disabled(self.loading || offset == 0)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.load_page(
                                        view.part.clone(),
                                        offset.saturating_sub(query::PAGE_ROWS),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                        .child(format!(
                            "{}–{}{}",
                            if count == 0 { 0 } else { offset + 1 },
                            offset + count,
                            total.map(|total| format!(" / {total}")).unwrap_or_default()
                        ))
                        .child(
                            Button::new("result-next")
                                .small()
                                .ghost()
                                .label("下一页")
                                .disabled(self.loading || !has_more)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.load_page(
                                        view.part.clone(),
                                        offset + query::PAGE_ROWS,
                                        window,
                                        cx,
                                    )
                                })),
                        )
                    })
                    .when(self.loading, |bar| bar.child("正在读取…"))
                    .when(self.error.is_some() && self.lease.is_none(), |bar| {
                        bar.child(
                            Button::new("retry-result")
                                .small()
                                .ghost()
                                .label("重试")
                                .on_click(cx.listener(|view, _, window, cx| view.load(window, cx))),
                        )
                    }),
            )
            .children(self.error.as_ref().map(|error| {
                div()
                    .px_2()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when_some(self.table.as_ref(), |body, table| {
                        body.child(DataTable::new(table).small().stripe(true))
                    })
                    .when(self.table.is_none(), |body| {
                        body.child(
                            uniform_list(
                                "result-values",
                                self.rows.len(),
                                cx.processor(|view, range: std::ops::Range<usize>, _, cx| {
                                    range.map(|index| view.render_row(index, cx)).collect()
                                }),
                            )
                            .size_full(),
                        )
                    }),
            )
    }
}

impl EventEmitter<PanelEvent> for ResultPanel {}
impl EventEmitter<ResultEvent> for ResultPanel {}
impl Focusable for ResultPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ResultPanel {
    fn panel_name(&self) -> &'static str {
        "result"
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(ResultEvent::Activated);
        }
    }
    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.closed = true;
        self.generation += 1;
        self.task = None;
        self.lease = None;
        self.table = None;
        self.value = None;
        self.rows.clear();
        cx.emit(ResultEvent::Closed);
    }
}

impl Panel for ResultPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(NativeIcon::Table).size_3())
            .child("结果")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
