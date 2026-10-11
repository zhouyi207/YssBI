//! Captured document saves preserve the edited snapshot even if disk persistence fails.
use super::DocumentEditor;
use crate::file_commands::{FileSaveOutcome, FileSaveRequest};
use gpui_kit::{Context, Window};
use yss_project_model::doc::{DocDocument, DocEdit};

pub(crate) type DocumentSaveRequest = FileSaveRequest<DocDocument>;
pub(crate) type DocumentSaveOutcome = FileSaveOutcome<DocDocument>;

impl DocumentEditor {
    pub fn cancel_prepared_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        self.autosave_in_flight = false;
        if self.refresh_again {
            self.refresh(window, cx);
        }
        self.resume_autosave(window, cx);
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
        self.autosave_task = None;
        self.autosave.save_started();
        self.autosave_in_flight = false;
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
        let was_autosave = self.autosave_in_flight;
        self.autosave_in_flight = false;
        if let Some(snapshot) = outcome.snapshot {
            self.snapshot = snapshot;
            self.draft_dirty = self.input.read(cx).text() != self.snapshot.content.0.as_str();
        }
        if outcome.failed {
            self.error = Some(
                crate::text::t(if was_autosave {
                    "preferences.documents.autosaveFailed"
                } else {
                    "native.documents.saveFailed"
                })
                .into(),
            );
            self.autosave.failed();
            self.autosave_task = None;
        }
        if self.refresh_again {
            self.refresh(window, cx);
        }
        self.resume_autosave(window, cx);
        cx.emit(gpui_kit::component::dock::PanelEvent::LayoutChanged);
        cx.emit(super::DocumentEvent::Changed);
        cx.notify();
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_document(false, window, cx);
    }

    pub(super) fn save_document(
        &mut self,
        automatic: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.prepare_save(cx) else {
            return;
        };
        // Explicit saves and file-operation locks remain readonly for close/save-all.
        // Autosaves capture a revision without interrupting continued typing.
        self.autosave_in_flight = automatic;
        let owner = self.services.clone();
        let job = self
            .services
            .run(move |services| Ok(request.commit(services, &owner)));
        self._save_task = Some(cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(DocumentSaveOutcome {
                    snapshot: None,
                    failed: true,
                });
            let _ = view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx));
        }));
    }

    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.refreshing {
            self.refresh_again = true;
            return;
        }
        self.refresh_again = false;
        self.autosave_task = None;
        self.refreshing = true;
        cx.notify();
        let project = self.snapshot.project_instance_id.clone();
        let path = self.snapshot.path.clone();
        let job = self
            .services
            .run(move |services| Ok(services.application.read_doc(project, path)?));
        self._refresh_task = Some(cx.spawn_in(window, async move |view, cx| {
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
                        view.error = Some(crate::text::t("native.documents.externalChange").into());
                        view.autosave.failed();
                    }
                    None => {
                        view.error = Some(crate::text::t("native.documents.readFailed").into());
                        view.autosave.failed();
                    }
                }
                if view.refresh_again {
                    view.refresh(window, cx);
                }
                view.resume_autosave(window, cx);
                cx.emit(super::DocumentEvent::Changed);
                cx.notify();
            });
        }));
    }
}
