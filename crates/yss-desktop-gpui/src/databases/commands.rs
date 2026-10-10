use super::{DatabaseEditor, DatabaseEvent};
use crate::services::NativeServices;
use gpui_kit::{Context, Window};
use std::sync::Arc;
use yss_application::{database::DatabaseMutation, runtime::ApplicationServices};
use yss_database_contract::EditState;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

pub(crate) struct DatabaseSaveRequest {
    project: ProjectInstanceId,
    id: String,
    revision: ResourceRevision,
}
pub(crate) struct DatabaseSaveOutcome {
    pub edit: Option<EditState>,
    pub failed: bool,
}
impl DatabaseSaveRequest {
    pub fn commit(
        self,
        services: &ApplicationServices,
        owner: &Arc<NativeServices>,
    ) -> DatabaseSaveOutcome {
        match services.application.save_database_for_application(
            self.project,
            self.id,
            self.revision,
            OperationId::new(),
        ) {
            Ok(receipt) => {
                owner.publish_resource(receipt.mutation);
                DatabaseSaveOutcome {
                    edit: Some(receipt.data),
                    failed: false,
                }
            }
            Err(_) => DatabaseSaveOutcome {
                edit: None,
                failed: true,
            },
        }
    }
}
impl DatabaseEditor {
    pub fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<DatabaseSaveRequest> {
        if self.busy() || !self.ready {
            return None;
        }
        self.mutating = true;
        self.error = None;
        self.changed(cx);
        Some(DatabaseSaveRequest {
            project: self.project.clone(),
            id: self.id.clone(),
            revision: self.revision,
        })
    }
    pub fn cancel_prepared_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mutating = false;
        if self.refresh_again {
            self.reload(true, window, cx);
        }
        self.changed(cx);
    }
    pub fn finish_save(&mut self, outcome: DatabaseSaveOutcome, cx: &mut Context<Self>) {
        self.mutating = false;
        if let Some(edit) = outcome.edit {
            self.edit = Some(edit);
        }
        self.ready = false;
        if outcome.failed {
            self.error = Some(crate::text::t("native.databases.saveFailed").into());
        }
        // Checkpoint/import can replace the Application session; the host must reattach its bindings.
        cx.emit(DatabaseEvent::SessionChanged);
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
                .unwrap_or(DatabaseSaveOutcome {
                    edit: None,
                    failed: true,
                });
            let _ = view.update_in(cx, |view, _, cx| view.finish_save(outcome, cx));
        })
        .detach();
    }
    pub(super) fn mutate(
        &mut self,
        mutation: DatabaseMutation,
        expected: ResourceRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || !self.ready || self.revision != expected {
            return;
        }
        self.mutating = true;
        self.error = None;
        let project = self.project.clone();
        let id = self.id.clone();
        let owner = self.services.clone();
        let job = self.services.run(move |services| {
            let receipt = services.application.mutate_database_for_application(
                project,
                id,
                expected,
                OperationId::new(),
                mutation,
            )?;
            owner.publish_resource(receipt.mutation);
            Ok(receipt.data.edit_state)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                view.mutating = false;
                match result {
                    Some(edit) => {
                        view.edit = Some(edit);
                        view.ready = false;
                    }
                    None => {
                        view.error =
                            Some(crate::text::t("native.databases.columnUpdateFailed").into());
                    }
                }
                if view.refresh_again {
                    view.reload(false, window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub(super) fn export(
        &mut self,
        format: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || !self.ready {
            return;
        }
        let revision = self.revision;
        let generation = self.generation;
        let name = format!("{}.{}", self.name, format);
        let directory = std::env::current_dir().unwrap_or_default();
        self.busy = true;
        self.changed(cx);
        let prompt = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn_in(window, async move |view, cx| {
            let path = prompt.await.ok().and_then(Result::ok).flatten();
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                if view.revision == revision
                    && view.ready
                    && let Some(path) = path
                {
                    view.export_to(path, format, window, cx);
                } else if view.refresh_again {
                    view.reload(true, window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
    }
    fn export_to(
        &mut self,
        path: std::path::PathBuf,
        format: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        let project = self.project.clone();
        let id = self.id.clone();
        let revision = self.revision;
        let job = self.services.run(move |services| {
            services.application.export_database_for_application(
                project,
                id,
                path.to_string_lossy().into_owned(),
                format.into(),
                Some(revision),
            )?;
            Ok(())
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                if result.is_none() {
                    view.error = Some(crate::text::t("native.databases.exportFailed").into());
                }
                if view.refresh_again {
                    view.reload(true, window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
}
