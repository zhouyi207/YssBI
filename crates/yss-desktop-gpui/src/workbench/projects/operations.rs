//! Commit project commands off-thread, then observe and install the actual authority.
use super::{ProjectCommand, ProjectOperation, ProjectStage};
use crate::{project::DesktopProject, workbench::Workbench};
use gpui::{Context, Window};
use gpui_component::WindowExt;
use std::path::PathBuf;
use yss_application::{
    events::{ProjectLifecycleApplicationEvent, ProjectLifecycleOutcome},
    project::query::ProjectQueryApplicationError,
    runtime::ApplicationServices,
};
use yss_project_identity::{OperationId, ProjectInstanceId};

struct ProjectObservation {
    active: Option<ProjectInstanceId>,
    project: Result<Option<DesktopProject>, anyhow::Error>,
    error: Option<String>,
    recovery: Option<PathBuf>,
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
            let project = services
                .application
                .query_project_index(identity.clone(), "zh-CN", true)
                .map(|index| Some(DesktopProject::new(identity.clone(), index)))
                .map_err(anyhow::Error::from);
            (Some(identity), project)
        }
        Err(ProjectQueryApplicationError::ProjectIdentityMismatch { .. }) => (None, Ok(None)),
        Err(error) => (None, Err(error.into())),
    }
}
fn lifecycle_receipt(receipt: &ProjectLifecycleApplicationEvent) -> Result<(), &'static str> {
    match receipt.outcome {
        ProjectLifecycleOutcome::Committed => Ok(()),
        ProjectLifecycleOutcome::RegistryFailed => {
            Err("项目文件已写入，但登记未完成；请打开已写入的项目。")
        }
        ProjectLifecycleOutcome::ActivationFailed => {
            Err("副本已写入并登记，但未打开；请打开已写入的项目。")
        }
        ProjectLifecycleOutcome::RegistryPending => {
            Err("项目已移到回收站，列表登记尚未清理；请刷新或清理失效记录。")
        }
    }
}
fn commit(
    command: ProjectCommand,
    expected: Option<yss_project_identity::ProjectInstanceId>,
    services: &ApplicationServices,
    executor: &tokio::runtime::Handle,
    progress: &tokio::sync::watch::Sender<ProjectStage>,
) -> ProjectObservation {
    let mut receipt = None;
    let register = matches!(&command, ProjectCommand::Open(_));
    let result = (|| -> anyhow::Result<()> {
        if let Some(expected) = &expected {
            anyhow::ensure!(
                services
                    .application
                    .capture_session()?
                    .project_instance_id()
                    == expected,
                "project session changed"
            );
        }
        match command {
            ProjectCommand::Open(path) => {
                services.application.load_project_for_application(
                    path.to_str()
                        .ok_or_else(|| anyhow::anyhow!("invalid path encoding"))?,
                )?;
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
                lifecycle_receipt(created).map_err(anyhow::Error::msg)?;
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
                        expected.ok_or_else(|| anyhow::anyhow!("no active project"))?,
                        OperationId::new(),
                    ),
                )?);
                lifecycle_receipt(receipt.as_ref().unwrap()).map_err(anyhow::Error::msg)?;
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
    let mut error = result.err().map(|_| {
        receipt
            .as_ref()
            .and_then(|receipt| lifecycle_receipt(receipt).err())
            .unwrap_or(if receipt.is_some() {
                "项目文件可能已写入，打开未完成。请检查目标目录后再继续。"
            } else {
                "项目操作未完成，原有输入已保留；请检查名称、目录或当前项目。"
            })
            .to_owned()
    });
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
            error = Some("项目已打开，但最近项目列表未更新。请刷新或重新扫描登记。".into());
        }
    }
    ProjectObservation {
        active,
        project,
        error,
        recovery,
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
            operation.fail(self.lifecycle, "请等待当前任务结束。", cx);
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
        self.lifecycle = self.lifecycle.wrapping_add(1);
        let lifecycle = self.lifecycle;
        window.focus(&self.focus, cx);
        self.event_task = None;
        self.graph_subscription = None;
        self.ui_binding = None;
        self.ui_delivery = None;
        self.intent_queue.clear();
        self.intent_busy = false;
        self.index_generation = self.index_generation.wrapping_add(1);
        self.refreshing_index = false;
        self.index_again = false;
        let executor = self.services.executor.clone();
        let watcher = self.services.clone();
        let dialog = operation.dialog;
        let job = self.services.run(move |services| {
            let observation = commit(
                operation.command,
                operation.project,
                services,
                &executor,
                &delivery,
            );
            if let Some(identity) = &observation.active {
                delivery.send_replace(ProjectStage::Watching);
                let _ = watcher.watch_project(identity);
            }
            Ok(observation)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.busy = false;
                view.project_progress = None;
                let (mut error, recovery) = if let Some(observation) = result {
                    match observation.project {
                        Ok(project) => view.install_project(project, window, cx),
                        Err(_) => {
                            if observation.active.as_ref().is_some_and(|active| {
                                view.project
                                    .as_ref()
                                    .is_none_or(|current| &current.identity != active)
                            }) {
                                // Activation is proven, even when its resource read failed.
                                view.install_project(None, window, cx);
                                view.error =
                                    Some("项目已切换，但内容未读取；请重新打开项目目录。".into());
                            } else {
                                view.connect_events(window, cx);
                                view.error =
                                    Some("项目状态未确认，输入已保留；请重新打开项目。".into());
                            }
                        }
                    }
                    (
                        observation.error.or_else(|| view.error.clone()),
                        observation.recovery,
                    )
                } else {
                    view.connect_events(window, cx);
                    (
                        Some("项目任务未完成，输入已保留；请检查当前目录。".into()),
                        None,
                    )
                };
                if let Some(dialog) = &dialog {
                    if error.is_none() {
                        window.close_dialog(cx);
                    } else {
                        let _ = dialog.update(cx, |form, cx| {
                            form.finish(lifecycle, error.clone(), recovery, cx)
                        });
                    }
                }
                view.error = error.take();
                view.recent.update(cx, |recent, cx| recent.reload(cx));
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn install_project(
        &mut self,
        project: Option<DesktopProject>,
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
        self.project = project;
        self.graphs.clear();
        self.conversations.clear();
        self.assistant_generation = self.assistant_generation.wrapping_add(1);
        self.assistant_reading = false;
        self.assistant_again = false;
        self.assistant_reveal = false;
        self.assistant_intent = None;
        self.documents.clear();
        self.charts.clear();
        self.minds.clear();
        self.databases.clear();
        self.result_panels.clear();
        self.opening.clear();
        self.subscriptions.clear();
        self.activities.clear();
        self.layout_task = None;
        self.layout_root = None;
        self.restoring_layout = false;
        self.reset_layout(window, cx);
        self.connect_panels(window, cx);
        self.install_activity(window, cx);
        self.connect_events(window, cx);
        self.restore_layout(None, window, cx);
        self.refresh_assistant_directory(false, window, cx);
    }
}
