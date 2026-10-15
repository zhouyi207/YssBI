//! Commit project commands off-thread, then observe and install the actual authority.
use super::{ProjectCommand, ProjectOperation, ProjectStage};
use crate::{project::DesktopProject, projects::feedback::ProjectFeedback, workbench::Workbench};
use gpui_kit::{Context, Window};
use std::path::PathBuf;
use yss_application::{
    events::{ProjectLifecycleApplicationEvent, ProjectLifecycleOutcome},
    project::{
        lifecycle::{ApplicationProjectLifecycleError, ProjectLifecycleError},
        query::ProjectQueryApplicationError,
    },
    runtime::ApplicationServices,
    session::{SessionCaptureError, SessionRevalidationError},
};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_ui::lifecycle::LogReleased;

struct ProjectObservation {
    active: Option<ProjectInstanceId>,
    project: Result<Option<DesktopProject>, anyhow::Error>,
    error: Option<ProjectFeedback>,
    recovery: Option<PathBuf>,
    opened_path: Option<String>,
}
fn read_project(
    services: &ApplicationServices,
) -> (
    Option<ProjectInstanceId>,
    anyhow::Result<Option<DesktopProject>>,
) {
    match services.application.query_current_project_activation() {
        Ok(activation) => {
            let identity = activation.project_instance_id;
            let language = crate::text::locale();
            let project = services
                .application
                .query_project_presentation(identity.clone(), language)
                .map(|index| Some(DesktopProject::new(identity.clone(), index, language)))
                .map_err(anyhow::Error::from);
            (Some(identity), project)
        }
        Err(ProjectQueryApplicationError::ProjectIdentityMismatch { .. }) => (None, Ok(None)),
        Err(error) => (None, Err(error.into())),
    }
}
fn require_committed(receipt: &ProjectLifecycleApplicationEvent) -> anyhow::Result<()> {
    anyhow::ensure!(
        receipt.outcome == ProjectLifecycleOutcome::Committed,
        "project lifecycle returned a partial commit"
    );
    Ok(())
}
fn commit(
    command: ProjectCommand,
    expected: Option<yss_project_identity::ProjectInstanceId>,
    services: &ApplicationServices,
    executor: &tokio::runtime::Handle,
    progress: &tokio::sync::watch::Sender<ProjectStage>,
) -> ProjectObservation {
    let mut receipt = None;
    let register = matches!(&command, ProjectCommand::Open { .. });
    let remember = matches!(
        &command,
        ProjectCommand::Open { .. } | ProjectCommand::Create { .. } | ProjectCommand::SaveAs(_)
    );
    let result = (|| -> anyhow::Result<()> {
        if let Some(expected) = &expected
            && services
                .application
                .capture_session()?
                .project_instance_id()
                != expected
        {
            return Err(ApplicationProjectLifecycleError::SessionChanged(
                SessionRevalidationError::Changed,
            )
            .into());
        }
        match command {
            ProjectCommand::Open { path, .. } => {
                services
                    .application
                    .load_project_for_application(path.to_str().ok_or(
                        ApplicationProjectLifecycleError::Lifecycle(
                            ProjectLifecycleError::InvalidPath,
                        ),
                    )?)?;
            }
            ProjectCommand::Create { name, destination } => {
                let created =
                    executor.block_on(services.application.create_project_for_application(
                        &services.projects,
                        &name,
                        &destination,
                        OperationId::new(),
                    ))?;
                receipt = Some(created);
                let created = receipt.as_ref().unwrap();
                require_committed(created)?;
                progress.send_replace(ProjectStage::Opening);
                services.application.load_project_for_application(
                    created
                        .path
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("created path unavailable"))?,
                )?;
            }
            ProjectCommand::SaveAs(destination) => {
                receipt = Some(executor.block_on(
                    services.application.save_project_as_for_application(
                        &services.projects,
                        &destination,
                        expected.ok_or(SessionCaptureError::Inactive)?,
                        OperationId::new(),
                    ),
                )?);
                require_committed(receipt.as_ref().unwrap())?;
            }
            ProjectCommand::Close => {
                if let Some(expected) = expected {
                    services
                        .application
                        .clear_project_for_application(&expected)?;
                }
                services.application.stop_project_watcher(&services.watcher);
            }
            ProjectCommand::Exit => unreachable!("window exit does not commit a project command"),
        }
        Ok(())
    })();
    let mut recovery = receipt
        .as_ref()
        .filter(|receipt| {
            matches!(
                receipt.outcome,
                ProjectLifecycleOutcome::RegistryFailed | ProjectLifecycleOutcome::ActivationFailed
            )
        })
        .and_then(|receipt| receipt.path.as_deref())
        .map(PathBuf::from);
    let mut error = result
        .err()
        .map(|error| ProjectFeedback::operation(&error, receipt.as_ref()));
    progress.send_replace(ProjectStage::Reading);
    let (active, project) = read_project(services);
    if recovery.is_none() && (error.is_some() || project.is_err()) {
        recovery = receipt
            .as_ref()
            .filter(|receipt| {
                matches!(
                    receipt.kind,
                    yss_application::events::ProjectLifecycleKind::Create
                        | yss_application::events::ProjectLifecycleKind::SaveAs
                )
            })
            .and_then(|receipt| receipt.path.as_deref())
            .map(PathBuf::from);
    }
    let opened_path = if remember && error.is_none() {
        match (
            &project,
            services.application.query_current_project_activation(),
        ) {
            (Ok(Some(project)), Ok(activation))
                if project.identity == activation.project_instance_id =>
            {
                Some(activation.path)
            }
            _ => None,
        }
    } else {
        None
    };
    if register
        && error.is_none()
        && let Ok(Some(project)) = &project
        && let Ok(activation) = services.application.query_current_project_activation()
    {
        progress.send_replace(ProjectStage::Registering);
        if executor
            .block_on(
                services
                    .projects
                    .register_project(&project.index.project_name, &activation.path),
            )
            .is_err()
        {
            error = Some(ProjectFeedback::message(
                "native.workbench.recentProjectsUpdateFailed",
            ));
        }
    }
    ProjectObservation {
        active,
        project,
        error,
        recovery,
        opened_path,
    }
}
impl Workbench {
    pub(in crate::workbench) fn perform_project_operation(
        &mut self,
        operation: ProjectOperation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Submission excluded active writes. Save receipts can start harmless rereads.
        if self.busy || self.closing {
            operation.fail(
                self.lifecycle,
                crate::text::t("native.workbench.waitForTask"),
                cx,
            );
            return;
        }
        if matches!(operation.command, ProjectCommand::Exit) {
            self.persist_layout(cx);
            window.remove_window();
            return;
        }
        self.persist_layout(cx);
        self.busy = true;
        let (progress, delivery) = operation.command.progress(cx);
        operation.show_progress(progress.clone(), cx);
        self.project_progress = Some(progress);
        self.error = None;
        self.assistant
            .update(cx, |assistant, cx| assistant.begin_project_transition(cx));
        self.lifecycle = self.lifecycle.wrapping_add(1);
        let lifecycle = self.lifecycle;
        window.focus(&self.focus, cx);
        self.event_task = None;
        self.graph_subscription = None;
        self.ui_binding = None;
        self.ui_delivery = None;
        self.intent_queue.clear();
        self.active_intent = None;
        self.index_generation = self.index_generation.wrapping_add(1);
        self.refreshing_index = false;
        self.index_again = false;
        let executor = self.services.executor.clone();
        let watcher = self.services.clone();
        let dialog = operation.dialog;
        let initial_resource = match &operation.command {
            ProjectCommand::Open { resource, .. } => resource.clone(),
            _ => None,
        };
        let job = self.services.run(move |services| {
            let mut observation = commit(
                operation.command,
                operation.project,
                services,
                &executor,
                &delivery,
            );
            if let Some(path) = observation.opened_path.take()
                && !matches!(
                    executor.block_on(watcher.layouts.remember_project(path)),
                    Ok(Ok(()))
                )
            {
                observation.error = Some(ProjectFeedback::message(
                    "preferences.startup.rememberFailed",
                ));
            }
            if let Some(identity) = &observation.active {
                delivery.send_replace(ProjectStage::Watching);
                let _ = watcher.watch_project(identity);
            }
            Ok(observation)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.busy = false;
                view.project_progress = None;
                let (mut error, recovery) = if let Some(observation) = result {
                    match observation.project {
                        Ok(project) => view.install_project(project, initial_resource, window, cx),
                        Err(_) => {
                            if observation.active.as_ref().is_some_and(|active| {
                                view.project
                                    .as_ref()
                                    .is_none_or(|current| &current.identity != active)
                            }) {
                                // Activation is proven, even when its resource read failed.
                                view.install_project(None, None, window, cx);
                                view.error = Some(
                                    crate::text::t("native.workbench.projectContentFailed").into(),
                                );
                            } else {
                                view.connect_events(window, cx);
                                view.refresh_assistant_directory(window, cx);
                                view.error = Some(
                                    crate::text::t("native.workbench.projectStateUnconfirmed")
                                        .into(),
                                );
                            }
                        }
                    }
                    (
                        observation
                            .error
                            .map(|error| error.text())
                            .or_else(|| view.error.clone()),
                        observation.recovery,
                    )
                } else {
                    view.connect_events(window, cx);
                    view.refresh_assistant_directory(window, cx);
                    (
                        Some(crate::text::t("native.workbench.projectTaskFailed").into()),
                        None,
                    )
                };
                if let Some(dialog) = &dialog {
                    if error.is_none() {
                        crate::modal_window::close_child(window, cx);
                    } else if dialog
                        .update(cx, |form, cx| {
                            form.finish(lifecycle, error.clone(), recovery, cx)
                        })
                        .log_released()
                        .is_some()
                    {
                        error = None;
                    }
                }
                view.error = error.take();
                view.recent.update(cx, |recent, cx| recent.reload(cx));
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }
    fn install_project(
        &mut self,
        project: Option<DesktopProject>,
        initial_resource: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let (Some(current), Some(next)) = (&self.project, &project)
            && current.identity == next.identity
        {
            self.install_project_index(project.unwrap(), window, cx);
            self.rebind_session(window, cx);
            return;
        }
        self.close_result_windows(cx);
        self.project = project;
        self.graphs.clear();
        self.assistant.update(cx, |assistant, cx| {
            assistant.reset(
                self.project
                    .as_ref()
                    .map(|project| project.identity.clone()),
                cx,
            );
            assistant.set_resource_catalog(
                self.project
                    .as_ref()
                    .map(|project| project.resources.clone()),
                cx,
            );
        });
        self.assistant_intent = None;
        self.documents.clear();
        self.charts.clear();
        self.minds.clear();
        self.databases.clear();
        self.result_panels.clear();
        self.opening.clear();
        self.graph_openings.clear();
        self.subscriptions.clear();
        self.activities.clear();
        self.layout_task = None;
        self.layout_root = None;
        self.restoring_layout = false;
        self.reset_layout(window, cx);
        self.connect_panels(window, cx);
        self.install_activity(window, cx);
        self.connect_events(window, cx);
        self.restore_layout(initial_resource, window, cx);
        self.refresh_assistant_directory(window, cx);
        // YSS-DEBUG-BEGIN
        if std::env::var_os("YSS_DEBUG_CONVERSATION").is_some() {
            cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(4))
                    .await;
                view.update_in(cx, |view, window, cx| view.new_conversation(window, cx))
                    .ok();
            })
            .detach();
        }
        // YSS-DEBUG-END
        // YSS-DEBUG-BEGIN
        if let Ok(spec) = std::env::var("YSS_DEBUG_BENCH") {
            cx.spawn_in(window, async move |_, cx| {
                use gpui_kit::{
                    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
                    ScrollDelta, ScrollWheelEvent, point, px,
                };
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(14))
                    .await;
                let parts: Vec<&str> = spec.split(':').collect();
                let mode = parts[0].to_string();
                let x: f32 = parts[1].parse().unwrap_or(0.);
                let y: f32 = parts[2].parse().unwrap_or(0.);
                if mode == "drag" {
                    cx.update(|window, cx| {
                        window.dispatch_event(
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position: point(px(x), px(y)),
                                pressed_button: None,
                                modifiers: Default::default(),
                            }),
                            cx,
                        );
                        window.draw(cx).clear(cx);
                        window.dispatch_event(
                            PlatformInput::MouseDown(MouseDownEvent {
                                button: MouseButton::Left,
                                position: point(px(x), px(y)),
                                modifiers: Default::default(),
                                click_count: 1,
                                first_mouse: false,
                            }),
                            cx,
                        );
                        window.draw(cx).clear(cx);
                    })
                    .ok();
                }
                let mut samples = Vec::new();
                for i in 0..80u32 {
                    let elapsed = cx.update(|window, cx| {
                        let wave = if (i / 20) % 2 == 0 {
                            (i % 20) as f32
                        } else {
                            (20 - i % 20) as f32
                        };
                        let wave = if i == 79 { 20. } else { wave };
                        let event = if mode == "drag" {
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position: point(px(x + wave * 8.), px(y)),
                                pressed_button: Some(MouseButton::Left),
                                modifiers: Default::default(),
                            })
                        } else {
                            PlatformInput::ScrollWheel(ScrollWheelEvent {
                                position: point(px(x), px(y)),
                                delta: ScrollDelta::Pixels(point(
                                    px(0.),
                                    px(if (i / 40) % 2 == 0 { 60. } else { -60. }),
                                )),
                                modifiers: Default::default(),
                                touch_phase: Default::default(),
                            })
                        };
                        window.dispatch_event(event, cx);
                        let start = std::time::Instant::now();
                        window.draw(cx).clear(cx);
                        start.elapsed()
                    });
                    if let Ok(elapsed) = elapsed {
                        samples.push(elapsed);
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                }
                if mode == "drag" {
                    cx.update(|window, cx| {
                        window.dispatch_event(
                            PlatformInput::MouseUp(MouseUpEvent {
                                button: MouseButton::Left,
                                position: point(px(x), px(y)),
                                modifiers: Default::default(),
                                click_count: 1,
                            }),
                            cx,
                        );
                    })
                    .ok();
                }
                samples.sort();
                let total: std::time::Duration = samples.iter().sum();
                eprintln!(
                    "YSS-BENCH {mode} n={} mean={:?} median={:?} p90={:?} max={:?}",
                    samples.len(),
                    total / samples.len().max(1) as u32,
                    samples[samples.len() / 2],
                    samples[samples.len() * 9 / 10],
                    samples[samples.len() - 1]
                );
            })
            .detach();
        }
        // YSS-DEBUG-END
        if self
            .project
            .as_ref()
            .is_some_and(|project| project.language != crate::text::locale())
        {
            self.refresh_project(window, cx);
        }
    }
}
