mod activity;
mod chrome;
mod controls;
mod input;
mod graph_properties;
mod resources;
pub(crate) use chrome::SaveAllGraphs;
mod details;
mod events;
mod graphs;
mod intents;
mod layout;
mod lifecycle;
mod logs;
mod output;
mod problems;
mod results;

use gpui::{Context, Entity, FocusHandle, IntoElement, Render, WeakEntity, Window, prelude::*, px};
use gpui_component::dock::{
    DockArea, DockLayout, DockPlacement, DockSkin, PanelStyle, panel_handle,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use crate::{canvas::GraphCanvas, project::DesktopProject, services::NativeServices};
use activity::{ActivityEvent, ActivityPanel};
use details::DetailsPanel;
use logs::LogsPanel;
use output::OutputPanel;
use problems::ProblemsPanel;
use results::ResultsPanel;

pub struct Workbench {
    services: Arc<NativeServices>,
    project: Option<DesktopProject>,
    dock: Entity<DockArea>,
    details: Entity<DetailsPanel>,
    problems: Entity<ProblemsPanel>,
    logs: Entity<LogsPanel>,
    output: Entity<OutputPanel>,
    results: Entity<ResultsPanel>,
    result_panels: BTreeMap<(uuid::Uuid, u64), WeakEntity<crate::results::ResultPanel>>,
    graphs: BTreeMap<String, WeakEntity<GraphCanvas>>,
    opening: BTreeSet<String>,
    subscriptions: Vec<gpui::Subscription>,
    activities: BTreeMap<&'static str, WeakEntity<ActivityPanel>>,
    event_task: Option<gpui::Task<()>>,
    graph_subscription: Option<yss_application::graph::editing::GraphActivitySubscription>,
    ui_binding: Option<yss_application::presentation::WorkbenchBinding>,
    intent_queue: VecDeque<yss_ui_contract::UiIntentReceipt>,
    intent_busy: bool,
    intent_resync: bool,
    refreshing_index: bool,
    index_again: bool,
    focus: FocusHandle,
    busy: bool,
    closing: bool,
    error: Option<String>,
    lifecycle: u64,
    layout_root: Option<String>,
    restoring_layout: bool,
    layout_subscription: Option<gpui::Subscription>,
    layout_task: Option<gpui::Task<()>>,
}

impl Workbench {
    pub fn new(
        services: Arc<NativeServices>,
        project: Option<DesktopProject>,
        initial_graph: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (dock, skin) = DockSkin::dock_area("yssbi-workbench", None, window, cx);
        skin.set_panel_style(PanelStyle::TabBar, cx);
        let details = cx.new(|cx| DetailsPanel::new(services.clone(), window, cx));
        let problems = cx.new(ProblemsPanel::new);
        let logs = cx.new(|cx| LogsPanel::new(services.clone(), window, cx));
        let output = cx.new(OutputPanel::new);
        let results = cx.new(ResultsPanel::new);
        let mut view = Self {
            services,
            project,
            dock,
            details,
            problems,
            logs,
            output,
            results,
            result_panels: BTreeMap::new(),
            graphs: BTreeMap::new(),
            opening: BTreeSet::new(),
            subscriptions: vec![],
            activities: BTreeMap::new(),
            event_task: None,
            graph_subscription: None,
            ui_binding: None,
            intent_queue: VecDeque::new(),
            intent_busy: false,
            intent_resync: false,
            refreshing_index: false,
            index_again: false,
            focus: cx.focus_handle(),
            busy: false,
            closing: false,
            error: None,
            lifecycle: 1,
            layout_root: None,
            restoring_layout: false,
            layout_subscription: None,
            layout_task: None,
        };
        view.install_default_layout(window, cx);
        view.install_activity(window, cx);
        view.connect_panels(window, cx);
        view.connect_events(window, cx);
        view.connect_layout(cx);
        cx.defer_in(window, move |view, window, cx| {
            view.restore_layout(initial_graph, window, cx)
        });
        view
    }

    fn connect_panels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.output,
            window,
            |view, _, event, window, cx| {
                let output::OutputEvent::Locate(source) = event;
                view.focus_graph(
                    source.graph().as_str().into(),
                    source.node().map(|node| node.as_str().into()),
                    window,
                    cx,
                );
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.results,
            window,
            |view, _, event, window, cx| {
                let results::ResultsEvent::Open(reference) = event;
                view.open_result(*reference, None, window, cx);
            },
        ));
    }

    fn install_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = &self.project else {
            return;
        };
        let mut left = DockLayout::tabs();
        for document in &project.panels {
            let panel = cx.new(|cx| ActivityPanel::new(document.clone(), cx));
            self.activities.insert(document.panel_id, panel.downgrade());
            self.subscriptions.push(cx.subscribe_in(
                &panel,
                window,
                |view, _, event, window, cx| match event {
                    ActivityEvent::OpenGraph(path) => view.open_graph(path.clone(), window, cx),
                    ActivityEvent::GraphResource(path, action) => {
                        view.graph_resource_action(path.clone(), *action, window, cx)
                    }
                    ActivityEvent::CreateNode(creation) => {
                        if let Some(graph) = view.details.read(cx).graph() {
                            graph.update(cx, |graph, cx| graph.create_node(creation.clone(), cx));
                        }
                    }
                },
            ));
            left = left.panel_view(panel_handle(panel), cx);
        }
        self.dock.update(cx, |dock, cx| {
            dock.set_dock(DockPlacement::Left, left, window, cx);
            dock.set_dock_size(DockPlacement::Left, px(260.), window, cx);
        });
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        self.graphs
            .values()
            .filter_map(WeakEntity::upgrade)
            .any(|graph| graph.read(cx).dirty())
    }

    pub fn is_closing(&self, cx: &gpui::App) -> bool {
        self.closing
            || self.busy
            || self
                .graphs
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|graph| graph.read(cx).busy())
    }
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_chrome(cx)
    }
}
