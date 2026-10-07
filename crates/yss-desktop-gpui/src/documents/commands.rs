//! Captured document saves preserve the edited snapshot even if disk persistence fails.
use super::DocumentEditor;
use crate::file_commands::{FileSaveOutcome, FileSaveRequest};
use gpui::{Context, Window};
use yss_project_model::doc::{DocDocument, DocEdit};

pub(crate) type DocumentSaveRequest = FileSaveRequest<DocDocument>;
pub(crate) type DocumentSaveOutcome = FileSaveOutcome<DocDocument>;

impl DocumentEditor {
    pub fn cancel_prepared_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        if self.refresh_again {
            self.refresh(window, cx);
        }
        cx.emit(super::DocumentEvent::Changed);
        cx.notify();
    }
    pub fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<DocumentSaveRequest> {
        if self.busy || self.refreshing {
            return None;
        }
        let request = DocumentSaveRequest {
            project: self.snapshot.project_instance_id.clone(),
            path: self.snapshot.path.clone(),
            version: self.snapshot.version.clone(),
            edits: if self.draft_dirty {
                vec![DocEdit::SetMarkdown {
                    markdown: self.input.read(cx).value().to_string(),
                }]
            } else {
                vec![]
            },
        };
        self.busy = true;
        self.error = None;
        cx.notify();
        Some(request)
    }

    pub fn finish_save(
        &mut self,
        outcome: DocumentSaveOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        if let Some(snapshot) = outcome.snapshot {
            self.snapshot = snapshot;
            self.draft_dirty = self.input.read(cx).text() != self.snapshot.content.0.as_str();
        }
        if outcome.failed {
            self.error = Some("文档未保存。内容已保留，请检查外部修改或文件写入错误。".into());
        }
        if self.refresh_again {
            self.refresh(window, cx);
        }
        cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
        cx.emit(super::DocumentEvent::Changed);
        cx.notify();
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.prepare_save(cx) else {
            return;
        };
        let owner = self.services.clone();
        let job = self
            .services
            .run(move |services| Ok(request.commit(services, &owner)));
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(DocumentSaveOutcome {
                    snapshot: None,
                    failed: true,
                });
            let _ = view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx));
        })
        .detach();
    }

    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.refreshing {
            self.refresh_again = true;
            return;
        }
        self.refresh_again = false;
        self.refreshing = true;
        cx.notify();
        let project = self.snapshot.project_instance_id.clone();
        let path = self.snapshot.path.clone();
        let job = self
            .services
            .run(move |services| Ok(services.application.read_doc(project, path)?));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                view.refreshing = false;
                match result {
                    Some(snapshot) if snapshot.version == view.snapshot.version => {}
                    Some(snapshot) if !view.draft_dirty => {
                        view.input.update(cx, |input, cx| {
                            input.set_value(snapshot.content.0.clone(), window, cx)
                        });
                        view.snapshot = snapshot;
                        view.draft_dirty = false;
                        view.error = None;
                        if view.preview_visible {
                            view.update_preview(cx);
                        }
                    }
                    Some(_) => {
                        view.error = Some(
                            "文档已在其他位置修改。当前输入已保留，保存需要重新核验版本。".into(),
                        )
                    }
                    None => {
                        view.error = Some("文档暂不可读取。当前输入已保留，请检查项目目录。".into())
                    }
                }
                if view.refresh_again {
                    view.refresh(window, cx);
                }
                cx.emit(super::DocumentEvent::Changed);
                cx.notify();
            });
        })
        .detach();
    }
}
