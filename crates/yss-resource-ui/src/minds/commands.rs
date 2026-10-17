use super::MindCanvas;
use crate::file_session::{FileEditor, FileSaveOutcome, FileSaveRequest, FileSession};
use gpui_kit::{Context, Window};
use yss_project::minds::MindCommand;
use yss_project_identity::OperationId;
use yss_project_model::mind::{MindDocument, MindEdit};
use yss_ui::lifecycle::LogReleased;

pub type MindSaveRequest = FileSaveRequest<MindDocument>;
pub type MindSaveOutcome = FileSaveOutcome<MindDocument>;

impl FileEditor for MindCanvas {
    type File = MindDocument;
    const PANEL_NAME: &'static str = "mind-editor";
    const PATH_KEY: &'static str = "mindPath";

    fn session(&self) -> &FileSession<MindDocument> {
        &self.file
    }

    fn session_mut(&mut self) -> &mut FileSession<MindDocument> {
        &mut self.file
    }

    fn has_draft(&self) -> bool {
        self.buffers.values().any(|buffer| buffer.dirty)
    }

    fn draft_edits(&mut self, cx: &mut Context<Self>) -> Option<Vec<MindEdit>> {
        self.pending_edits(cx)
    }

    fn snapshot_replaced(
        &mut self,
        _keep_draft: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reconcile_buffers(window, cx);
    }

    // Topic buffers keep their captured versions and resolve changes topic by topic.
    fn conflicts_with_draft(&self) -> bool {
        false
    }
}

impl MindCanvas {
    pub(super) fn apply_edits(
        &mut self,
        edits: Vec<MindEdit>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.file.editable() {
            return;
        }
        let Some(mut batch) = self.pending_edits(cx) else {
            return;
        };
        let selection = self.selection_after(&edits);
        let submitted = self.submitted_inputs();
        batch.extend(edits);
        if batch.is_empty() || !self.file.begin_commit() {
            return;
        }
        let snapshot = self.file.snapshot();
        let project = snapshot.project_instance_id.clone();
        let command = MindCommand::Edit {
            path: snapshot.path.clone(),
            version: snapshot.version.clone(),
            edits: batch,
        };
        self.cancel_gesture();
        let owner = self.file.services().clone();
        let job = self.file.services().run(move |services| {
            let receipt = services.apply_file_command(project, OperationId::new(), command)?;
            owner.publish_resource(receipt.mutation);
            Ok(receipt.snapshot)
        });
        let task = cx.spawn_in(window, async move |view, cx| {
            let snapshot = job.await.ok().and_then(Result::ok).flatten();
            view.update_in(cx, |view, window, cx| {
                let committed = snapshot.is_some();
                if committed {
                    view.acknowledge_inputs(&submitted);
                    view.follow_selection(selection);
                }
                view.finish_commit(snapshot, window, cx);
                if !committed {
                    view.file
                        .report(yss_ui::text::t("native.minds.topicSubmitFailed"), cx);
                }
            })
            .log_released();
        });
        self.file.set_commit_task(task);
        self.changed(cx);
    }
}
