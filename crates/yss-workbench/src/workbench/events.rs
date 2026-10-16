use super::{Workbench, activity::ReadState};
use crate::project::DesktopProject;
use gpui_kit::{Context, Window};
use tokio::sync::broadcast::error::RecvError;
use yss_application::graph::editing::GraphActivity;
use yss_desktop_services::DesktopEvent;
use yss_graph_document::GraphResourcePath;
use yss_ui::lifecycle::LogReleased;

impl Workbench {
    pub(super) fn connect_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_stale_result_windows(cx);
        self.event_task = None;
        self.graph_subscription = None;
        self.ui_binding = None;
        self.ui_delivery = None;
        self.intent_queue.clear();
        self.active_intent = None;
        self.intent_resync = false;
        self.assistant_intent = None;
        for opening in self
            .graph_openings
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
        {
            opening.update(cx, |opening, _| opening.clear_intent());
        }
        let Some(project) = &self.project else {
            return;
        };
        let mut receiver = self.services.subscribe();
        match self.services.graph_subscription(&project.identity) {
            Ok(subscription) => self.graph_subscription = Some(subscription),
            Err(_error) => tracing::error!(
                code = "native_graph_activity_attachment_failed",
                "Native graph activity attachment failed"
            ),
        }
        let delivery = uuid::Uuid::new_v4();
        match self.services.workbench_binding(&project.identity, delivery) {
            Ok(binding) => {
                self.ui_binding = Some(binding);
                self.ui_delivery = Some(delivery);
            }
            Err(_error) => tracing::error!(
                code = "native_workbench_attachment_failed",
                "Native workbench attachment failed"
            ),
        }
        self.event_task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if view
                            .update_in(cx, |view, window, cx| view.accept_event(event, window, cx))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => {
                        if view
                            .update_in(cx, |view, window, cx| {
                                view.close_stale_result_windows(cx);
                                view.refresh_project(window, cx);
                                view.refresh_graphs(cx);
                                view.refresh_documents(window, cx);
                                view.refresh_minds(window, cx);
                                for graph in view
                                    .graphs
                                    .values()
                                    .filter_map(gpui_kit::WeakEntity::upgrade)
                                {
                                    graph.update(cx, |graph, cx| graph.resync_execution(cx));
                                }
                                view.resync_intents(window, cx);
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }));
        self.resync_intents(window, cx);
    }

    fn accept_event(&mut self, event: DesktopEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = &self.project else {
            return;
        };
        match event {
            DesktopEvent::Graph(identity, GraphActivity::Execution(event)) if identity == project.identity => {
                if let yss_application::graph::run::RunApplicationEventKind::ResultInspectionRequested { result_id, .. } = event.kind() {
                    self.open_result(yss_graph_execution::result::ResultReference {
                        execution_session_id: *event.identity().execution_session_id(), result_id: *result_id,
                    }, None, None, window, cx);
                } else if let Some(graph) = self.graphs.get(event.identity().graph_path().as_str()).and_then(gpui_kit::WeakEntity::upgrade) {
                    graph.update(cx, |graph, cx| graph.accept_execution(event, cx));
                }
            }
            DesktopEvent::Graph(identity, GraphActivity::Changed { graph_path, cause, .. })
                if identity == project.identity =>
            {
                let resource = GraphResourcePath::new(graph_path.clone()).ok()
                    .map(yss_project_history::ResourceKey::Graph);
                for graph in self.graphs.values().filter_map(gpui_kit::WeakEntity::upgrade) {
                    let canvas = graph.read(cx);
                    let own_graph = canvas.path() == graph_path;
                    let dependency = resource.as_ref()
                        .is_some_and(|resource| canvas.graph().depends_on_resource(resource));
                    if own_graph {
                        graph.update(cx, |graph, cx| graph.accept_graph_change(&cause, cx));
                    } else if dependency {
                        graph.update(cx, |graph, cx| graph.refresh(cx));
                    }
                }
            }
            DesktopEvent::Index(invalidation)
                if invalidation.project_instance_id() == &project.identity =>
            {
                self.refresh_project(window, cx);
                if let Some(resources) = invalidation.resources() {
                    for graph in self.graphs.values().filter_map(gpui_kit::WeakEntity::upgrade) {
                        let canvas = graph.read(cx);
                        if resources.iter().any(|resource| {
                            matches!(resource, yss_project_history::ResourceKey::Graph(path) if path.as_str() == canvas.path())
                                || canvas.graph().depends_on_resource(resource)
                        }) {
                            graph.update(cx, |graph, cx| graph.refresh(cx));
                        }
                    }
                    for resource in resources {
                        if let yss_project_history::ResourceKey::Doc(path) = resource
                            && let Some(document) = self.documents.get(path.0.as_ref()).and_then(gpui_kit::WeakEntity::upgrade) {
                            document.update(cx, |document, cx| document.refresh(window, cx));
                        }
                        if let yss_project_history::ResourceKey::Mind(path) = resource
                            && let Some(mind) = self.minds.get(path.0.as_ref()).and_then(gpui_kit::WeakEntity::upgrade) {
                            mind.update(cx, |mind, cx| mind.refresh(window, cx));
                        }
                    }
                } else {
                    self.refresh_graphs(cx);
                    self.refresh_documents(window, cx);
                    self.refresh_minds(window, cx);
                }
            }
            DesktopEvent::Resource(mutation) if mutation.project_instance_id == project.identity => {
                self.accept_graph_resources(&mutation, window, cx);
                if !self.refreshing_index
                    && self.project.as_mut().is_some_and(|project| project.accept_file_save(&mutation))
                {
                    let resources = self.project.as_ref().map(|project| project.resources.clone());
                    self.assistant.update(cx, |assistant, cx| {
                        assistant.set_resource_catalog(resources, cx);
                    });
                } else {
                    self.refresh_project(window, cx);
                }
                for delta in &mutation.deltas {
                    if let yss_project_history::ResourceKey::Doc(path) = &delta.resource
                        && let Some(document) = self.documents.get(path.0.as_ref()).and_then(gpui_kit::WeakEntity::upgrade) {
                        document.update(cx, |document, cx| document.refresh(window, cx));
                    }
                    if let yss_project_history::ResourceKey::Mind(path) = &delta.resource
                        && let Some(mind) = self.minds.get(path.0.as_ref()).and_then(gpui_kit::WeakEntity::upgrade) {
                        mind.update(cx, |mind, cx| mind.refresh(window, cx));
                    }
                }
            }
            DesktopEvent::Ui(identity, delivery, event)
                if identity == project.identity && self.ui_delivery == Some(delivery) => match event {
                yss_ui_contract::UiEvent::Intent { receipt } => {
                    self.enqueue_intent(receipt, window, cx)
                }
                yss_ui_contract::UiEvent::Resync => self.resync_intents(window, cx),
                yss_ui_contract::UiEvent::SessionChanged => {
                    self.rebind_session(window, cx);
                }
            },
            DesktopEvent::Harness(event) => {
                tracing::trace!(session_id=?event.session_id,sequence=event.sequence,"Native Harness event delivered");
            }
            _ => {}
        }
    }

    pub(super) fn refresh_graphs(&self, cx: &mut Context<Self>) {
        for graph in self
            .graphs
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
        {
            graph.update(cx, |graph, cx| graph.refresh(cx));
        }
    }

    pub(super) fn rebind_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_assistant_directory(window, cx);
        self.assistant.update(cx, |assistant, cx| {
            assistant.reload_conversations(window, cx);
        });
        self.connect_events(window, cx);
        self.refresh_project(window, cx);
        self.refresh_graphs(cx);
        for chart in self
            .charts
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
        {
            chart.update(cx, |chart, cx| chart.refresh(window, cx));
        }
        for editor in self
            .databases
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
        {
            editor.update(cx, |editor, cx| editor.reload(false, window, cx));
        }
        for graph in self
            .graphs
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
        {
            graph.update(cx, |graph, cx| graph.resync_execution(cx));
        }
    }

    pub(super) fn install_project_index(
        &mut self,
        project: DesktopProject,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.index_generation = self.index_generation.wrapping_add(1);
        self.refreshing_index = false;
        self.assistant.update(cx, |assistant, cx| {
            assistant.set_resource_catalog(Some(project.resources.clone()), cx);
        });
        if project.language != crate::text::locale() {
            // Keep committed resource facts, but do not replace translated panels with an old locale.
            self.project = Some(project);
            self.refresh_project(window, cx);
            return;
        }
        let language_changed = self
            .project
            .as_ref()
            .is_none_or(|previous| previous.language != project.language);
        for document in &project.panels {
            if let Some(panel) = self
                .activities
                .get(document.panel_id)
                .and_then(gpui_kit::WeakEntity::upgrade)
            {
                let owner = cx.entity().downgrade();
                panel.update(cx, |panel, cx| {
                    panel.replace_document(document.clone(), cx);
                    if language_changed {
                        cx.notify();
                    }
                    if let Some(catalog) = project
                        .panels
                        .iter()
                        .find(|panel| panel.panel_id == "nodes")
                    {
                        panel.set_project_resources(owner, catalog.clone(), cx);
                    }
                });
            }
            if document.panel_id == "nodes" {
                for graph in self
                    .graphs
                    .values()
                    .filter_map(gpui_kit::WeakEntity::upgrade)
                {
                    graph.update(cx, |graph, cx| {
                        graph.set_catalog(document.clone(), &project.language, cx)
                    });
                }
            }
        }
        self.project = Some(project);
        self.refresh_databases(window, cx);
        self.refresh_charts(window, cx);
    }

    pub(super) fn refresh_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing_index {
            self.index_again = true;
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        self.refreshing_index = true;
        self.activity_read_state(&["project", "nodes"], ReadState::Loading, cx);
        self.index_generation = self.index_generation.wrapping_add(1);
        let generation = self.index_generation;
        self.index_again = false;
        let identity = project.identity.clone();
        let lifecycle = self.lifecycle;
        let query = identity.clone();
        let language = crate::text::locale();
        let task = self.services.run(move |services| {
            Ok(DesktopProject::new(
                query.clone(),
                services
                    .application
                    .query_project_presentation(query, language)?,
                language,
            ))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view.index_generation != generation
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != identity)
                {
                    return;
                }
                view.refreshing_index = false;
                if language != crate::text::locale() {
                    view.refresh_project(window, cx);
                    return;
                }
                match result {
                    Ok(project) => view.install_project_index(project, window, cx),
                    Err(_error) => {
                        view.activity_read_state(
                            &["project", "nodes"],
                            ReadState::Failed("native.workbench.activityLoadFailed"),
                            cx,
                        );
                        let lifecycle = view.lifecycle;
                        yss_ui::notifications::Notifications::new(window).error_with_retry(
                            crate::text::translate("native.workbench.activityLoadFailed"),
                            cx,
                            move |view, window, cx| {
                                if view.lifecycle == lifecycle {
                                    view.refresh_project(window, cx);
                                }
                            },
                        );
                        tracing::warn!(
                            code = "native_activity_refresh_failed",
                            "Native activity refresh failed"
                        );
                    }
                }
                if view.index_again {
                    view.refresh_project(window, cx);
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
    }
}
