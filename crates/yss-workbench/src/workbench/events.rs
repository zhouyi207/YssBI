use super::{Workbench, activity::ReadState, files::FileKind};
use crate::project::{DesktopProject, FileIndexUpdate};
use gpui_kit::{Context, Window};
use tokio::sync::broadcast::error::RecvError;
use yss_application::events::{CommittedResourceMutation, ResourceRoute};
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
                                view.resync_editors(window, cx);
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
                match invalidation.resources() {
                    Some(resources) => {
                        for editor in self.editors().collect::<Vec<_>>() {
                            if resources.iter().any(|resource| editor.shows(resource, cx)) {
                                editor.refresh(window, cx);
                            }
                        }
                    }
                    None => self.resync_editors(window, cx),
                }
            }
            DesktopEvent::Resource(mutation) if mutation.project_instance_id == project.identity => {
                let refreshing = self.refreshing_index;
                let update = match self.project.as_mut() {
                    Some(project) if !refreshing => project.accept_file_receipt(&mutation),
                    _ => FileIndexUpdate::Rejected,
                };
                match update {
                    FileIndexUpdate::Rejected => self.refresh_project(window, cx),
                    FileIndexUpdate::Revisions | FileIndexUpdate::Rows => {
                        let resources = self.project.as_ref().map(|project| project.resources.clone());
                        self.assistant.update(cx, |assistant, cx| {
                            assistant.set_resource_catalog(resources, cx);
                        });
                        if matches!(update, FileIndexUpdate::Rows) {
                            self.publish_activity_documents(false, cx);
                        }
                    }
                }
                self.accept_resource_mutation(&mutation, window, cx);
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

    /// The single route for committed resource changes, whether the GUI, the Assistant or
    /// another writer made them: moves follow the open editor, deletions retire or detach it.
    pub(super) fn accept_resource_mutation(
        &mut self,
        mutation: &CommittedResourceMutation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .project
            .as_ref()
            .is_none_or(|project| project.identity != mutation.project_instance_id)
        {
            return;
        }
        let mut moved_files = false;
        for route in mutation.routes() {
            match route {
                ResourceRoute::Moved {
                    from,
                    to,
                    from_revision,
                    to_revision,
                } => {
                    if FileKind::of(to).is_some() {
                        moved_files |=
                            self.move_file(from, to, from_revision, to_revision, window, cx);
                    } else if let Ok(to) = GraphResourcePath::new(to) {
                        self.move_graph_resource(from, to, from_revision, to_revision, window, cx);
                    }
                }
                ResourceRoute::Removed { path, revision } => {
                    if let Some(panel) = self.file_panel(path) {
                        panel.mark_removed(cx);
                    } else if GraphResourcePath::new(path).is_ok() {
                        self.remove_graph_resource(path, revision, window, cx);
                    }
                }
                ResourceRoute::Changed { path, delta } => {
                    if let Some(panel) = self.file_panel(path) {
                        panel.accept_change(delta, window, cx);
                    }
                }
            }
        }
        if moved_files {
            self.persist_layout(cx);
        }
        // Graph projections also follow their dependencies and the graphs that a
        // signature change invalidated, not only their own resource.
        let affected = mutation.projection_status.affected_graph_paths();
        for graph in self
            .graphs
            .values()
            .filter_map(gpui_kit::WeakEntity::upgrade)
        {
            let canvas = graph.read(cx);
            if affected.iter().any(|path| path.as_str() == canvas.path())
                || mutation
                    .deltas
                    .iter()
                    .any(|delta| canvas.shows(&delta.resource))
            {
                graph.update(cx, |graph, cx| graph.refresh(cx));
            }
        }
    }

    fn resync_editors(&self, window: &mut Window, cx: &mut Context<Self>) {
        for editor in self.editors().collect::<Vec<_>>() {
            editor.resync(window, cx);
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
        self.resync_editors(window, cx);
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
        self.project = Some(project);
        self.publish_activity_documents(language_changed, cx);
        let editors = self.editors().collect::<Vec<_>>();
        if let Some(project) = &self.project {
            for editor in editors {
                editor.accept_index(project, window, cx);
            }
        }
    }

    /// Hands the accepted activity documents to their panels; project rows bind the node catalog.
    fn publish_activity_documents(&self, notify: bool, cx: &mut Context<Self>) {
        let Some(project) = &self.project else {
            return;
        };
        let nodes = project
            .panels
            .iter()
            .find(|panel| panel.panel_id == "nodes")
            .cloned();
        for document in &project.panels {
            if let Some(panel) = self
                .activities
                .get(document.panel_id)
                .and_then(gpui_kit::WeakEntity::upgrade)
            {
                let owner = cx.entity().downgrade();
                let nodes = nodes.clone();
                panel.update(cx, |panel, cx| {
                    panel.replace_document(document.clone(), cx);
                    if notify {
                        cx.notify();
                    }
                    if let Some(nodes) = nodes {
                        panel.set_project_resources(owner, nodes, cx);
                    }
                });
            }
        }
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
