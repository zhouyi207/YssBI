//! Native result panels own view state and one Application result lease.
mod addition;
mod plot;
mod query;
mod reading;
mod render;
mod report;
mod table;
mod toolbar;
mod value;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Window,
    div, prelude::*,
};
use gpui_component::{
    Icon,
    dock::{BasePanel, Panel, PanelEvent},
    table::TableState,
};
use std::{collections::BTreeSet, sync::Arc};
use yss_application::graph::results::report::ResultTablePart;
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

use crate::services::NativeServices;
use gpui_kit_assets::IconName;
use query::{ResultContent, ResultLease};
use table::ResultGrid;

pub enum ResultEvent {
    Loaded(bool),
    Activated,
    Closed,
    Replaced {
        previous: ResultReference,
        current: ResultReference,
    },
}

pub struct ResultPanel {
    services: Arc<NativeServices>,
    reference: ResultReference,
    lease: Option<ResultLease>,
    focus: FocusHandle,
    value: Option<Arc<RuntimeValue>>,
    plot: Option<Entity<plot::PlotView>>,
    report: Option<Entity<report::ReportView>>,
    report_mode: bool,
    rows: Vec<value::ValueRow>,
    expanded: BTreeSet<String>,
    tables: bool,
    table: Option<Entity<TableState<ResultGrid>>>,
    part: Option<ResultTablePart>,
    loading: bool,
    error: Option<reading::ReadFailure>,
    view_locale: &'static str,
    generation: u64,
    task: Option<gpui::Task<()>>,
    closed: bool,
    report_subscription: Option<gpui::Subscription>,
    addition_task: Option<gpui::Task<()>>,
    addition_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
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
            plot: None,
            report: None,
            report_mode: false,
            rows: vec![],
            expanded: BTreeSet::new(),
            tables: false,
            table: None,
            part: None,
            loading: false,
            error: None,
            view_locale: crate::text::locale(),
            generation: 0,
            task: None,
            closed: false,
            report_subscription: None,
            addition_task: None,
            addition_cancel: None,
        }
    }
    pub fn available(&self) -> bool {
        !self.closed
    }
    pub fn loaded(&self) -> bool {
        self.lease.is_some()
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
        self.cancel_addition();
        self.addition_task = None;
        self.report_subscription = None;
        self.generation += 1;
        self.task = None;
        self.replace_lease(None);
        self.table = None;
        self.value = None;
        self.plot = None;
        self.report = None;
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
            .child(
                Icon::new(if self.plot.is_some() {
                    IconName::ChartLine
                } else {
                    IconName::Table
                })
                .size_3(),
            )
            .child("结果")
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
