use super::MindCanvas;
use crate::file_commands::{FileSaveOutcome, FileSaveRequest};
use gpui::{Context, Window};
use yss_project::minds::MindCommand;
use yss_project_identity::OperationId;
use yss_project_model::mind::{MindDocument, MindEdit, MindNode};

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
            self.error = Some("思维导图未保存。当前内容已保留，请检查外部修改或写入错误。".into());
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
                    view.install_snapshot(snapshot, window, cx);
                } else {
                    view.error = Some("主题修改未提交。输入已保留，请检查当前文件。".into());
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
                    None => view.error = Some("思维导图暂不可读取。当前内容已保留。".into()),
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
    pub(super) fn add_topic(&mut self, sibling: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(topic) = self.selected_topic() else {
            return;
        };
        let parent_id = if sibling {
            let Some(parent) = &topic.parent_id else {
                return;
            };
            parent.clone()
        } else {
            topic.id.clone()
        };
        let node = MindNode {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: Some(parent_id),
            content: "新主题".into(),
            reference: None,
        };
        self.apply_edits(vec![MindEdit::AddNode { node }], window, cx);
    }
    pub(super) fn delete_topics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let node_ids = self
            .selected
            .iter()
            .filter(|id| *id != &self.snapshot.content.root_id)
            .cloned()
            .collect::<Vec<_>>();
        if !node_ids.is_empty() {
            self.apply_edits(vec![MindEdit::RemoveNodes { node_ids }], window, cx);
        }
    }
    pub(super) fn reorder_topic(
        &mut self,
        direction: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(topic) = self.selected_topic() else {
            return;
        };
        let Some(parent) = &topic.parent_id else {
            return;
        };
        let siblings = self
            .snapshot
            .content
            .nodes
            .iter()
            .filter(|node| node.parent_id.as_ref() == Some(parent))
            .collect::<Vec<_>>();
        let Some(index) = siblings.iter().position(|node| node.id == topic.id) else {
            return;
        };
        let target = index as isize + direction;
        if target < 0 || target >= siblings.len() as isize {
            return;
        }
        let before_id = if direction < 0 {
            Some(siblings[target as usize].id.clone())
        } else {
            siblings
                .get(target as usize + 1)
                .map(|node| node.id.clone())
        };
        let edit = MindEdit::MoveNode {
            node_id: topic.id.clone(),
            parent_id: parent.clone(),
            before_id,
        };
        self.apply_edits(vec![edit], window, cx);
    }
}
