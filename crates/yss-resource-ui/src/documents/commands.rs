//! Captured document saves preserve the edited snapshot even if disk persistence fails.
use super::DocumentEditor;
use crate::file_session::{
    FileEditor, FileFailure, FileSaveOutcome, FileSaveRequest, FileSession, report_failure,
};
use gpui_kit::{Context, Window};
use yss_project_model::doc::{DocDocument, DocEdit};

pub type DocumentSaveRequest = FileSaveRequest<DocDocument>;
pub type DocumentSaveOutcome = FileSaveOutcome<DocDocument>;

impl FileEditor for DocumentEditor {
    type File = DocDocument;

    fn session(&self) -> &FileSession<DocDocument> {
        &self.file
    }

    fn session_mut(&mut self) -> &mut FileSession<DocDocument> {
        &mut self.file
    }

    fn has_draft(&self) -> bool {
        self.draft_dirty
    }

    fn draft_edits(&mut self, cx: &mut Context<Self>) -> Option<Vec<DocEdit>> {
        self.autosave_task = None;
        self.autosave.save_started();
        Some(if self.draft_dirty {
            vec![DocEdit::SetMarkdown {
                markdown: self.input.read(cx).value().to_string(),
            }]
        } else {
            vec![]
        })
    }

    fn snapshot_replaced(&mut self, keep_draft: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !keep_draft && self.input.read(cx).text() != self.file.snapshot().content.0.as_str() {
            let content = self.file.snapshot().content.0.clone();
            self.input
                .update(cx, |input, cx| input.set_value(content, window, cx));
        }
        self.draft_dirty = self.input.read(cx).text() != self.file.snapshot().content.0.as_str();
        if self.preview_visible {
            self.update_preview(cx);
        }
    }

    fn session_settled(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave_in_flight = false;
        self.resume_autosave(window, cx);
    }

    fn session_failed(&mut self, failure: FileFailure, cx: &mut Context<Self>) {
        if failure == FileFailure::Save && self.autosave_in_flight {
            self.file
                .report(yss_ui::text::t("preferences.documents.autosaveFailed"), cx);
        } else {
            report_failure(self, failure, cx);
        }
        // Failed writes and unresolved versions wait for another edit or an explicit save.
        self.autosave.failed();
        self.autosave_task = None;
    }
}

impl DocumentEditor {
    pub(super) fn save_document(
        &mut self,
        automatic: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_committing = self.file.committing();
        self.save(window, cx);
        // Explicit saves and file-operation locks remain readonly for close/save-all.
        // Autosaves capture a revision without interrupting continued typing.
        if !was_committing && self.file.committing() {
            self.autosave_in_flight = automatic;
        }
    }
}
