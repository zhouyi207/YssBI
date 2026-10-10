use super::MindCanvas;
use crate::file_commands::{FileSaveOutcome, FileSaveRequest};
use gpui::{Context, Window};
use yss_project::minds::MindCommand;
use yss_project_identity::OperationId;
use yss_project_model::mind::{MindDocument, MindEdit};

pub(crate) type MindSaveRequest = FileSaveRequest<MindDocument>;
pub(crate) type MindSaveOutcome = FileSaveOutcome<MindDocument>;

impl MindCanvas {
    pub fn cancel_prepared_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = false;
        if self.refresh_again {
            self.refresh(window, cx);
        }
        self.changed(cx);
    }
    pub fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<MindSaveRequest> {
        if self.busy() {
            return None;
        }
        let edits = self.pending_edits(cx)?;
        self.busy = true;
        self.error = None;
        self.changed(cx);
        Some(MindSaveRequest {
            project: self.snapshot.project_instance_id.clone(),
            path: self.snapshot.path.clone(),
            version: self.snapshot.version.clone(),
            edits,
        })
    }
    pub fn finish_save(
        &mut self,
        outcome: MindSaveOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        if let Some(snapshot) = outcome.snapshot {
            self.install_snapshot(snapshot, window, cx);
        }
        if outcome.failed {
            self.error = Some(crate::text::t("native.minds.saveFailed").into());
        }
        if self.refresh_again {
            self.refresh(window, cx);
        }
        self.changed(cx);
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
                .unwrap_or(MindSaveOutcome {
                    snapshot: None,
                    failed: true,
                });
            let _ = view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx));
        })
        .detach();
    }
    pub(super) fn apply_edits(
        &mut self,
        edits: Vec<MindEdit>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        let Some(mut batch) = self.pending_edits(cx) else {
            return;
        };
        let selection = self.selection_after(&edits);
        let submitted = self.submitted_inputs();
        batch.extend(edits);
        if batch.is_empty() {
            return;
        }
        let project = self.snapshot.project_instance_id.clone();
        let command = MindCommand::Edit {
            path: self.snapshot.path.clone(),
            version: self.snapshot.version.clone(),
            edits: batch,
        };
        self.busy = true;
        self.error = None;
        self.cancel_gesture();
        let owner = self.services.clone();
        let job = self.services.run(move |services| {
            let receipt =
                services
                    .application
                    .apply_mind_command(project, OperationId::new(), command)?;
            owner.publish_resource(receipt.mutation);
            Ok(receipt.snapshot)
        });
        cx.spawn_in(window, async move |view, cx| {
            let snapshot = job.await.ok().and_then(Result::ok).flatten();
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                if let Some(snapshot) = snapshot {
                    view.acknowledge_inputs(&submitted);
                    view.follow_selection(selection);
                    view.install_snapshot(snapshot, window, cx);
                } else {
                    view.error = Some(crate::text::t("native.minds.topicSubmitFailed").into());
                }
                if view.refresh_again {
                    view.refresh(window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            self.refresh_again = true;
            return;
        }
        self.refresh_again = false;
        self.refreshing = true;
        let project = self.snapshot.project_instance_id.clone();
        let path = self.snapshot.path.clone();
        let job = self
            .services
            .run(move |services| Ok(services.application.read_mind(project, path)?));
        cx.spawn_in(window, async move |view, cx| {
            let snapshot = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                view.refreshing = false;
                match snapshot {
                    Some(snapshot) if snapshot.version == view.snapshot.version => {}
                    Some(snapshot) => view.install_snapshot(snapshot, window, cx),
                    None => view.error = Some(crate::text::t("native.minds.readFailed").into()),
                }
                if view.refresh_again {
                    view.refresh(window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
}
