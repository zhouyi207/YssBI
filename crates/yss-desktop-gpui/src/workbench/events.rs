use super::Workbench;
use crate::{project::DesktopProject, services::NativeEvent};
use gpui::{Context, Window};
use tokio::sync::broadcast::error::RecvError;
use yss_application::graph::editing::GraphActivity;

impl Workbench {
    pub(super) fn connect_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.event_task = None;
        self.graph_subscription = None;
        self.ui_binding = None;
        self.intent_queue.clear();
        self.intent_busy = false;
        self.intent_resync = false;
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
        match self.services.workbench_binding(&project.identity) {
            Ok(binding) => self.ui_binding = Some(binding),
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
                                view.refresh_project(window, cx);
                                view.refresh_graphs(cx);
                                for graph in
                                    view.graphs.values().filter_map(gpui::WeakEntity::upgrade)
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

    fn accept_event(&mut self, event: NativeEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = &self.project else {
            return;
        };
        match event {
            NativeEvent::Graph(identity, GraphActivity::Execution(event)) if identity == project.identity => {
                if let yss_application::graph::run::RunApplicationEventKind::ResultInspectionRequested { result_id, .. } = event.kind() {
                    self.open_result(yss_graph_execution::result::ResultReference {
                        execution_session_id: *event.identity().execution_session_id(), result_id: *result_id,
                    }, None, window, cx);
                } else if let Some(graph) = self.graphs.get(event.identity().graph_path().as_str()).and_then(gpui::WeakEntity::upgrade) {
                    graph.update(cx, |graph, cx| graph.accept_execution(event, cx));
                }
            }
            NativeEvent::Graph(identity, GraphActivity::Changed { graph_path, .. })
                if identity == project.identity =>
            {
                if let Some(graph) = self
                    .graphs
                    .get(&graph_path)
                    .and_then(gpui::WeakEntity::upgrade)
                {
                    graph.update(cx, |graph, cx| graph.refresh(cx));
                }
            }
            NativeEvent::Index(invalidation)
                if invalidation.project_instance_id() == &project.identity =>
            {
                self.refresh_project(window, cx);
                self.refresh_graphs(cx);
            }
            NativeEvent::Resource(mutation) if mutation.project_instance_id == project.identity => {
                self.refresh_project(window, cx);
                self.refresh_graphs(cx);
            }
            NativeEvent::Ui(identity, event) if identity == project.identity => match event {
                yss_ui_contract::UiEvent::Intent { receipt } => {
                    self.enqueue_intent(receipt, window, cx)
                }
                yss_ui_contract::UiEvent::Resync => self.resync_intents(window, cx),
                yss_ui_contract::UiEvent::SessionChanged => {
                    self.ui_binding = None;
                    self.intent_queue.clear();
                }
            },
            NativeEvent::Harness(event) => {
                tracing::trace!(session_id=?event.session_id,sequence=event.sequence,"Native Harness event delivered");
            }
            _ => {}
        }
    }

    fn refresh_graphs(&self, cx: &mut Context<Self>) {
        for graph in self.graphs.values().filter_map(gpui::WeakEntity::upgrade) {
            graph.update(cx, |graph, cx| graph.refresh(cx));
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
        self.index_again = false;
        let identity = project.identity.clone();
        let lifecycle = self.lifecycle;
        let query = identity.clone();
        let task = self.services.run(move |services| {
            Ok(DesktopProject::new(
                query.clone(),
                services
                    .application
                    .query_project_index(query, "zh-CN", true)?,
            ))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != identity)
                {
                    return;
                }
                view.refreshing_index = false;
                match result {
                    Ok(project) => {
                        for document in &project.panels {
                            if let Some(panel) = view
                                .activities
                                .get(document.panel_id)
                                .and_then(gpui::WeakEntity::upgrade)
                            {
                                panel.update(cx, |panel, cx| {
                                    panel.replace_document(document.clone(), cx)
                                });
                            }
                            if document.panel_id == "nodes" {
                                for graph in
                                    view.graphs.values().filter_map(gpui::WeakEntity::upgrade)
                                {
                                    graph.update(cx, |graph, cx| {
                                        graph.set_catalog(document.clone(), cx)
                                    });
                                }
                            }
                        }
                        view.project = Some(project);
                    }
                    Err(_error) => tracing::warn!(
                        code = "native_activity_refresh_failed",
                        "Native activity refresh failed"
                    ),
                }
                if view.index_again {
                    view.refresh_project(window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
