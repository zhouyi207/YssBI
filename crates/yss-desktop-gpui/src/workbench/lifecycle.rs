//! Shared saving and window closure; project navigation resumes after successful saves.
use super::{
    Workbench,
    projects::{ProjectCommand, ProjectOperation},
};
use gpui::{AppContext, Context, Window};

pub(super) enum AfterSave {
    Stay,
    Project(Box<ProjectOperation>),
}
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
    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let operation = ProjectOperation::new(ProjectCommand::Exit, None, self);
        self.request_project_operation(operation, window, cx);
    }
    pub(super) fn save_all(
        &mut self,
        after: AfterSave,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) {
            self.error = Some("文件仍在读取或提交，请稍后再试。".into());
            if let AfterSave::Project(operation) = &after {
                operation.fail(self.lifecycle, "文件仍在读取或提交，请稍后再试。", cx);
            }
            cx.notify();
            return;
        }
        let Some((targets, requests)) = self.capture_saves(window, cx) else {
            if let AfterSave::Project(operation) = &after {
                operation.fail(self.lifecycle, "保存未准备完成，输入已保留。", cx);
            }
            return;
        };
        self.closing = matches!(after, AfterSave::Project(_));
        self.busy = true;
        self.error = None;
        let lifecycle = self.lifecycle;
        window.focus(&self.focus, cx);
        let owner = self.services.clone();
        let task = self.services.run(move |services| {
            Ok(requests
                .into_iter()
                .map(|request| request.commit(services, &owner))
                .collect::<Vec<_>>())
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle {
                    return;
                }
                view.busy = false;
                view.closing = false;
                let failed = if let Some(responses) = result {
                    targets
                        .iter()
                        .zip(responses)
                        .fold(false, |failed, (target, response)| {
                            target.finish(response, window, cx) || failed
                        })
                } else {
                    for target in &targets {
                        target.fail(window, cx);
                    }
                    true
                };
                if failed {
                    view.error = Some("部分文件未保存，请逐一检查。".into());
                }
                if let AfterSave::Project(operation) = after {
                    if !failed && !view.has_unsaved(cx) {
                        view.perform_project_operation(*operation, window, cx);
                    } else {
                        operation.fail(
                            view.lifecycle,
                            "保存未完成，项目操作已停止，输入已保留。",
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn reset_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.details = cx.new(|cx| super::DetailsPanel::new(self.services.clone(), window, cx));
        self.problems = cx.new(super::ProblemsPanel::new);
        self.output = cx.new(super::OutputPanel::new);
        self.results = cx.new(super::ResultsPanel::new);
        self.install_default_layout(window, cx);
    }
}
