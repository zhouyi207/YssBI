mod activity;
mod assistant;
mod charts;
mod chrome;
mod controls;
mod graph_properties;
mod input;
mod resources;
pub(crate) use chrome::{OpenProjectDirectory, OpenRecentProject, SaveAllGraphs, ShowSettings};
mod databases;
mod details;
mod documents;
mod events;
mod graphs;
mod imports;
mod intents;
mod layout;
mod lifecycle;
mod logs;
mod menus;
mod minds;
mod output;
mod plugins;
mod problems;
pub(crate) mod projects;
mod results;
mod saving;
mod settings;
mod status_bar;
mod welcome;

use gpui::{
    Context, Entity, FocusHandle, IntoElement, Render, WeakEntity, Window, WindowHandle,
    prelude::*, px,
};
use gpui_component::Root;
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
    recent: Entity<crate::projects::RecentProjects>,
    recent_subscription: Option<gpui::Subscription>,
    recent_picker:
        Option<WeakEntity<gpui_component::list::ListState<crate::projects::RecentDelegate>>>,
    plugins: Entity<crate::plugins::PluginsPanel>,
    plugin_subscription: Option<gpui::Subscription>,
    settings: Entity<crate::settings::SettingsPanel>,
    settings_window: Option<WindowHandle<Root>>,
    settings_subscription: Option<gpui::Subscription>,
    menu_bar: Entity<gpui_component::menu::AppMenuBar>,
    menu_context: Option<menus::MenuContext>,
    conversations: BTreeMap<String, Entity<crate::assistant::ConversationPanel>>,
    assistant_generation: u64,
    assistant_reading: bool,
    assistant_again: bool,
    assistant_reveal: bool,
    assistant_intent: Option<String>,
    assistant_busy: bool,
    dock: Entity<DockArea>,
    details: Entity<DetailsPanel>,
    problems: Entity<ProblemsPanel>,
    logs: Entity<LogsPanel>,
    output: Entity<OutputPanel>,
    results: Entity<ResultsPanel>,
    result_panels: BTreeMap<(uuid::Uuid, u64), WeakEntity<crate::results::ResultPanel>>,
    graphs: BTreeMap<String, WeakEntity<GraphCanvas>>,
    documents: BTreeMap<String, WeakEntity<crate::documents::DocumentEditor>>,
    charts: BTreeMap<String, WeakEntity<crate::charts::ChartEditor>>,
    minds: BTreeMap<String, WeakEntity<crate::minds::MindCanvas>>,
    databases: BTreeMap<String, WeakEntity<crate::databases::DatabaseEditor>>,
    opening: BTreeSet<String>,
    subscriptions: Vec<gpui::Subscription>,
    activities: BTreeMap<&'static str, WeakEntity<ActivityPanel>>,
    event_task: Option<gpui::Task<()>>,
    graph_subscription: Option<yss_application::graph::editing::GraphActivitySubscription>,
    ui_binding: Option<yss_application::presentation::WorkbenchBinding>,
    ui_delivery: Option<uuid::Uuid>,
    intent_queue: VecDeque<yss_ui_contract::UiIntentReceipt>,
    intent_busy: bool,
    intent_resync: bool,
    refreshing_index: bool,
    index_generation: u64,
    index_again: bool,
    focus: FocusHandle,
    busy: bool,
    project_progress: Option<Entity<crate::projects::progress::ProjectProgress>>,
    closing: bool,
    error: Option<String>,
    pub(crate) lifecycle: u64,
    layout_root: Option<String>,
    restoring_layout: bool,
    layout_subscription: Option<gpui::Subscription>,
    layout_task: Option<gpui::Task<()>>,
}

impl Workbench {
    pub fn new(
        services: Arc<NativeServices>,
        project: Option<DesktopProject>,
        initial_resource: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (dock, skin) = DockSkin::dock_area("yssbi-workbench", None, window, cx);
        skin.set_panel_style(PanelStyle::TabBar, cx);
        skin.set_close_button_visible(true, cx);
        skin.set_toggle_button_visible(false, cx);
        let details = cx.new(|cx| DetailsPanel::new(services.clone(), window, cx));
        let problems = cx.new(ProblemsPanel::new);
        let logs = cx.new(|cx| LogsPanel::new(services.clone(), window, cx));
        let output = cx.new(OutputPanel::new);
        let results = cx.new(ResultsPanel::new);
        let recent = cx.new(|_| crate::projects::RecentProjects::new(services.clone()));
        let plugins = cx.new(|cx| crate::plugins::PluginsPanel::new(services.clone(), window, cx));
        let settings = cx.new(|cx| crate::settings::SettingsPanel::new(services.clone(), cx));
        let menu_bar = gpui_component::menu::AppMenuBar::new(cx);
        let mut view = Self {
            services,
            project,
            recent,
            recent_subscription: None,
            recent_picker: None,
            plugins,
            plugin_subscription: None,
            settings,
            settings_window: None,
            settings_subscription: None,
            menu_bar,
            menu_context: None,
            conversations: BTreeMap::new(),
            assistant_generation: 0,
            assistant_reading: false,
            assistant_again: false,
            assistant_reveal: false,
            assistant_intent: None,
            assistant_busy: false,
            dock,
            details,
            problems,
            logs,
            output,
            results,
            result_panels: BTreeMap::new(),
            graphs: BTreeMap::new(),
            documents: BTreeMap::new(),
            charts: BTreeMap::new(),
            minds: BTreeMap::new(),
            databases: BTreeMap::new(),
            opening: BTreeSet::new(),
            subscriptions: vec![],
            activities: BTreeMap::new(),
            event_task: None,
            graph_subscription: None,
            ui_binding: None,
            ui_delivery: None,
            intent_queue: VecDeque::new(),
            intent_busy: false,
            intent_resync: false,
            refreshing_index: false,
            index_generation: 0,
            index_again: false,
            focus: cx.focus_handle(),
            busy: false,
            project_progress: None,
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
        view.connect_recent(window, cx);
        view.plugin_subscription = Some(cx.observe(&view.plugins, |_, _, cx| cx.notify()));
        view.settings_subscription = Some(cx.observe(&view.settings, |_, _, cx| cx.notify()));
        cx.on_release(|view, cx| {
            if let Some(handle) = view.settings_window {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        })
        .detach();
        cx.defer_in(window, move |view, window, cx| {
            view.recent
                .update(cx, |recent, cx| recent.reload(window, cx));
            if window.focused(cx).is_none() {
                window.focus(&view.focus, cx);
            }
            view.restore_layout(initial_resource, window, cx)
        });
        cx.defer_in(window, |view, window, cx| {
            view.refresh_assistant_directory(false, window, cx)
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
                    ActivityEvent::OpenDocument(path) => {
                        view.open_document(path.clone(), None, window, cx)
                    }
                    ActivityEvent::OpenMind(path) => view.open_mind(path.clone(), None, window, cx),
                    ActivityEvent::OpenDatabase(id) => {
                        view.open_database(id.clone(), None, window, cx)
                    }
                    ActivityEvent::OpenChart(path) => {
                        view.open_chart(path.clone(), None, window, cx)
                    }
                    ActivityEvent::ChartResource(path, action) => {
                        view.chart_resource_action(path.clone(), *action, window, cx)
                    }
                    ActivityEvent::GraphResource(path, action) => {
                        view.graph_resource_action(path.clone(), *action, window, cx)
                    }
                    ActivityEvent::DatabaseResource(id, action) => {
                        view.database_resource_action(id.clone(), *action, window, cx)
                    }
                    ActivityEvent::ImportData => view.import_dialog(window, cx),
                    ActivityEvent::OpenConversation(id) => {
                        view.open_conversation(id.clone(), window, cx)
                    }
                    ActivityEvent::RenameConversation(id, title) => {
                        view.rename_conversation(id.clone(), title.clone(), window, cx)
                    }
                    ActivityEvent::Tool(id) if id == "newConversation" => {
                        view.new_conversation(window, cx)
                    }
                    ActivityEvent::Tool(_) => {}
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
            dock.set_dock_size(DockPlacement::Left, px(240.), window, cx);
        });
    }

    fn activate_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.problems.update(cx, |problems, cx| problems.clear(cx));
        self.output
            .update(cx, |output, cx| output.set_graph(None, cx));
        self.results.update(cx, |results, cx| {
            results.set_graph(None);
            cx.notify();
        });
        for panel in self.activities.values().filter_map(WeakEntity::upgrade) {
            panel.update(cx, |panel, cx| panel.set_active_resource(Some(&path), cx));
        }
    }

    pub fn has_unsaved(&self, cx: &gpui::App) -> bool {
        if self.settings.read(cx).dirty() {
            return true;
        }
        if self
            .charts
            .values()
            .filter_map(WeakEntity::upgrade)
            .any(|chart| chart.read(cx).dirty())
        {
            return true;
        }
        self.graphs
            .values()
            .filter_map(WeakEntity::upgrade)
            .any(|graph| graph.read(cx).dirty())
            || self
                .documents
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|document| document.read(cx).dirty())
            || self
                .minds
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|mind| mind.read(cx).dirty())
            || self
                .databases
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|editor| editor.read(cx).dirty())
    }

    pub fn is_closing(&self, cx: &gpui::App) -> bool {
        self.closing
            || self.busy
            || self.plugins.read(cx).busy()
            || self.settings.read(cx).busy()
            || self.assistant_busy
            || self
                .conversations
                .values()
                .any(|view| view.read(cx).submitting())
            || self
                .charts
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|chart| chart.read(cx).busy())
            || self
                .graphs
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|graph| graph.read(cx).busy())
            || self
                .documents
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|document| document.read(cx).busy())
            || self
                .minds
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|mind| mind.read(cx).busy())
            || self
                .databases
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|editor| editor.read(cx).busy())
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_menus(cx);
        self.render_chrome(window, cx)
    }
}
