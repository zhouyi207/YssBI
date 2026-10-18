//! First reads belong to their tab; closing it cancels delivery without reopening it later.
//! Opens and unloads of one graph run in request order, so a close followed by a reopen
//! cannot release the graph under its new editor.
use super::super::Workbench;
use super::{Opening, opening::OpeningEvent};
use gpui_kit::component::dock::{PanelId, panel_handle};
use gpui_kit::{AppContext, Context, Entity, Focusable, Window};
use yss_application::graph::open::OpenGraphRequest;
use yss_graph_document::GraphResourcePath;
use yss_graph_ui::OpenedGraph;
use yss_project_identity::ProjectInstanceId;
use yss_ui::lifecycle::LogReleased;

impl Workbench {
    pub(in crate::workbench) fn create_graph_opening(
        &mut self,
        path: GraphResourcePath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Opening> {
        let revision = self.indexed_revision(path.as_str());
        let opening = cx.new(|cx| Opening::new(path, revision, cx));
        let lifecycle = self.lifecycle;
        self.subscriptions.push(cx.subscribe_in(
            &opening,
            window,
            move |view, opening, event, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                let path = opening.read(cx).path.as_str().to_owned();
                if view
                    .graph_openings
                    .get(&path)
                    .is_none_or(|held| held.entity_id() != opening.entity_id())
                {
                    return;
                }
                match event {
                    OpeningEvent::Removed(intent) => {
                        view.graph_openings.remove(&path);
                        // A read still in flight would leave the graph resident without a tab.
                        if let Some(project) = &view.project {
                            let project = project.identity.clone();
                            view.unload_graph(project, opening.read(cx).path.clone(), cx);
                        }
                        if let Some(intent) = intent {
                            view.finish_intent(intent, false, window, cx);
                        }
                    }
                    OpeningEvent::Activated => {
                        if view
                            .displayed_panel_placement(opening.entity_id().into(), cx)
                            .is_some()
                        {
                            view.clear_graph_context(cx);
                            view.mark_project_resource(Some(&path), cx);
                        }
                        if !opening.read(cx).failed {
                            view.read_graph_opening(opening.clone(), window, cx);
                        }
                    }
                    OpeningEvent::Retry => {
                        if !view.is_closing(cx) {
                            view.read_graph_opening(opening.clone(), window, cx);
                        }
                    }
                }
            },
        ));
        self.graph_openings.insert(
            opening.read(cx).path.as_str().to_owned(),
            opening.downgrade(),
        );
        opening
    }

    pub(super) fn read_graph_opening(
        &mut self,
        opening: Entity<Opening>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if opening.read(cx).removed || opening.read(cx).task.is_some() {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let identity = project.identity.clone();
        let lifecycle = self.lifecycle;
        let path = opening.read(cx).path.clone();
        let language = crate::text::locale();
        let request = OpenGraphRequest::new(identity.clone(), path.clone(), language);
        let services = self.services.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        // The job outlives a closed tab, so the unload queued behind it sees the loaded graph.
        self.queue_graph_job(&path, cx, async move {
            let result = services
                .run(move |services| {
                    Ok(OpenedGraph::from_open(
                        services.application.open_graph(request)?,
                        language,
                    ))
                })
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            if sender.send(result).is_err() {
                tracing::debug!(
                    code = "native_graph_open_unclaimed",
                    "Graph tab closed before its read finished"
                );
            }
        });
        let target = opening.downgrade();
        let delivery = cx.spawn_in(window, async move |view, cx| {
            let Ok(result) = receiver.await else {
                return;
            };
            view.update_in(cx, |view, window, cx| {
                let Some(opening) = target.upgrade() else {
                    return;
                };
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != identity)
                    || opening.read(cx).removed
                    || opening.read(cx).path != path
                    || view
                        .graph_openings
                        .get(path.as_str())
                        .is_none_or(|held| held.entity_id() != opening.entity_id())
                    || view
                        .panel_placement(PanelId::from(opening.entity_id()), cx)
                        .is_none()
                {
                    return;
                }
                opening.update(cx, |opening, _| opening.task = None);
                if language != crate::text::locale() {
                    view.read_graph_opening(opening, window, cx);
                    return;
                }
                let displayed = view
                    .displayed_panel_placement(PanelId::from(opening.entity_id()), cx)
                    .is_some();
                let focused = opening.focus_handle(cx).contains_focused(window, cx);
                let canvas = result
                    .ok()
                    .and_then(|graph| view.build_graph(graph, window, cx));
                let intent = opening.update(cx, |opening, _| opening.intent.take());
                let applied = if let Some(canvas) = canvas {
                    let node = opening.read(cx).node.clone();
                    view.graph_openings.remove(path.as_str());
                    let installed = view.replace_panel(
                        opening.clone(),
                        panel_handle(canvas.clone()),
                        window,
                        cx,
                    );
                    if installed && displayed && (focused || intent.is_some()) {
                        canvas.update(cx, |canvas, cx| {
                            canvas.focus_node(node.as_deref(), window, cx)
                        })
                    } else {
                        installed && displayed && node.is_none()
                    }
                } else {
                    tracing::warn!(
                        code = "native_graph_open_failed",
                        "Native graph open failed"
                    );
                    opening.update(cx, |opening, cx| {
                        opening.failed = true;
                        yss_ui::notifications::Notifications::new(window).error_with_retry(
                            crate::text::t("native.canvas.openFailed"),
                            cx,
                            |opening, _, cx| {
                                if !opening.removed && opening.failed {
                                    cx.emit(OpeningEvent::Retry);
                                }
                            },
                        );
                        cx.notify();
                    });
                    false
                };
                if let Some(intent) = intent {
                    view.finish_intent(&intent, applied, window, cx);
                }
                cx.notify();
            })
            .log_released();
        });
        opening.update(cx, |opening, cx| {
            opening.failed = false;
            opening.task = Some(delivery);
            cx.notify();
        });
    }

    /// Releases a graph without an editor; one with unsaved edits stays resident.
    pub(in crate::workbench) fn unload_graph(
        &mut self,
        project: ProjectInstanceId,
        path: GraphResourcePath,
        cx: &mut Context<Self>,
    ) {
        let services = self.services.clone();
        let owner = cx.weak_entity();
        let queued = path.clone();
        let job = cx.spawn(async move |_, cx| {
            let edited = owner
                .update(cx, |view, cx| view.graph_has_editor(path.as_str(), cx))
                .unwrap_or(true);
            if edited {
                return;
            }
            let result = services
                .run(move |services| {
                    Ok(services
                        .application
                        .unload_graph_resource(project, path, None)?)
                })
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            if let Err(error) = result {
                // Replacing the project retires its graphs before their tabs report removal.
                tracing::debug!(code = "native_graph_unload_skipped", %error, "Graph unload skipped");
            }
        });
        self.queue_graph_job(&queued, cx, job);
    }

    fn graph_has_editor(&self, path: &str, cx: &gpui_kit::App) -> bool {
        self.graphs
            .get(path)
            .and_then(gpui_kit::WeakEntity::upgrade)
            .is_some_and(|canvas| {
                self.panel_placement(canvas.entity_id().into(), cx)
                    .is_some()
            })
            || self
                .graph_openings
                .get(path)
                .and_then(gpui_kit::WeakEntity::upgrade)
                .is_some_and(|opening| !opening.read(cx).removed)
    }

    fn queue_graph_job(
        &mut self,
        path: &GraphResourcePath,
        cx: &mut Context<Self>,
        job: impl std::future::Future<Output = ()> + 'static,
    ) {
        let previous = self.graph_jobs.remove(path.as_str());
        let task = cx.spawn(async move |_, _| {
            if let Some(previous) = previous {
                previous.await;
            }
            job.await;
        });
        self.graph_jobs.insert(path.as_str().to_owned(), task);
    }
}
