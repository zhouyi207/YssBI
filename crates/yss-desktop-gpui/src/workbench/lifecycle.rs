use gpui::AppContext;
use gpui::{ClickEvent, Context, PathPromptOptions, PromptLevel, Window};
use yss_application::graph::editing::GraphEditRequest;
use yss_project_identity::OperationId;

use super::Workbench;
use crate::project::DesktopProject;

impl Workbench {
    pub fn close_requested(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.is_closing(cx) {
            return false;
        }
        if !self.has_unsaved(cx) {
            self.persist_layout(cx);
            return true;
        }
        self.request_close(window, cx);
        false
    }
    pub(super) fn choose_project(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing {
            return;
        }
        if self.has_unsaved(cx) {
            self.error = Some("请先保存当前图，再切换项目。".into());
            cx.notify();
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择 YssBI 项目目录".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = view.update_in(cx, |view, window, cx| view.open_project(path, window, cx));
            }
        })
        .detach();
    }

    fn open_project(
        &mut self,
        path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closing || self.has_unsaved(cx) {
            return;
        }
        self.busy = true;
        self.persist_layout(cx);
        self.error = None;
        window.focus(&self.focus, cx);
        cx.notify();
        let owner = self.services.clone();
        let task = self.services.run(move |services| {
            let activation = services
                .application
                .load_project_for_application(&path.to_string_lossy())?;
            let index = services
                .application
                .query_project_index(activation.project_instance_id.clone(), "zh-CN", true)
                .map_err(anyhow::Error::from);
            if let Err(_error) = owner.watch_project(&activation.project_instance_id) {
                tracing::warn!(
                    code = "project_activated_without_a_watcher",
                    "Project activated without a watcher"
                );
            }
            Ok((activation.project_instance_id, index))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                match result {
                    Ok((identity, index)) => {
                        view.lifecycle += 1;
                        view.project = match index {
                            Ok(index) => Some(DesktopProject::new(identity, index)),
                            Err(_error) => {
                                tracing::error!(
                                    code = "activated_project_index_read_failed",
                                    "Activated project index read failed"
                                );
                                view.error =
                                    Some("项目已切换，但索引读取失败。请重新打开该目录。".into());
                                None
                            }
                        };
                        view.graphs.clear();
                        view.result_panels.clear();
                        view.opening.clear();
                        view.subscriptions.clear();
                        view.activities.clear();
                        view.refreshing_index = false;
                        view.index_again = false;
                        view.reset_layout(window, cx);
                        view.connect_panels(window, cx);
                        view.install_activity(window, cx);
                        view.connect_events(window, cx);
                        view.restore_layout(None, window, cx);
                    }
                    Err(_error) => {
                        tracing::error!(
                            code = "native_project_open_failed",
                            "Native project open failed"
                        );
                        view.error = Some("无法打开项目，请检查所选目录。".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.closing || self.busy {
            return;
        }
        let prompt = window.prompt(
            PromptLevel::Warning,
            "有未保存的图",
            Some("退出前如何处理这些更改？"),
            &["保存并退出", "放弃更改", "取消"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(choice) = prompt.await {
                let _ = view.update_in(cx, |view, window, cx| match choice {
                    0 => view.save_all(true, window, cx),
                    1 => {
                        view.persist_layout(cx);
                        window.remove_window();
                    }
                    _ => {}
                });
            }
        })
        .detach();
    }

    pub(super) fn save_all(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.closing {
            return;
        }
        let graphs = self
            .graphs
            .values()
            .filter_map(gpui::WeakEntity::upgrade)
            .filter(|graph| graph.read(cx).dirty())
            .collect::<Vec<_>>();
        let requests = graphs
            .iter()
            .map(|graph| {
                let graph = &graph.read(cx).graph;
                GraphEditRequest {
                    project_instance_id: graph.project.clone(),
                    graph_path: graph.projection.graph_path.clone(),
                    version: graph.editing.version,
                    operation_id: OperationId::new(),
                    locale: "zh-CN".into(),
                }
            })
            .collect::<Vec<_>>();
        self.closing = close;
        self.busy = true;
        window.focus(&self.focus, cx);
        cx.notify();
        let task = self.services.run(move |services| {
            Ok(requests
                .into_iter()
                .map(|request| {
                    services
                        .application
                        .save_current_graph(request)
                        .map(|saved| saved.graph)
                        .map_err(anyhow::Error::from)
                })
                .collect::<Vec<_>>())
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                view.closing = false;
                match result {
                    Ok(responses) => {
                        let mut failed = false;
                        for (graph, response) in graphs.iter().zip(responses) {
                            match response {
                                Ok(response) => graph
                                    .update(cx, |graph, cx| graph.install_response(response, cx)),
                                Err(_error) => {
                                    failed = true;
                                    tracing::error!(
                                        code = "native_graph_save_failed",
                                        "Native graph save failed"
                                    );
                                }
                            }
                        }
                        if failed {
                            view.error = Some("部分图未保存，窗口保持打开，请逐图检查。".into());
                        }
                        if close && !failed {
                            view.persist_layout(cx);
                            window.remove_window();
                        }
                    }
                    Err(_error) => {
                        tracing::error!(code = "native_save_all_failed", "Native save all failed");
                        view.error = Some("保存未完成，窗口保持打开，请逐图检查。".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.details = cx.new(|cx| super::DetailsPanel::new(self.services.clone(), window, cx));
        self.problems = cx.new(super::ProblemsPanel::new);
        self.output = cx.new(super::OutputPanel::new);
        self.results = cx.new(super::ResultsPanel::new);
        self.install_default_layout(window, cx);
    }
}
