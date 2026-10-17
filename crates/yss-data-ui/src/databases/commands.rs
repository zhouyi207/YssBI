use super::{DatabaseEditor, DatabaseEvent};
use gpui_kit::{Context, Window};
use yss_application::database::DatabaseMutation;
use yss_database_contract::EditState;
use yss_desktop_services::ApplicationTasks;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_ui::lifecycle::LogReleased;

pub struct DatabaseSaveRequest {
    project: ProjectInstanceId,
    id: String,
    revision: ResourceRevision,
}
pub struct DatabaseSaveOutcome {
    edit: Option<EditState>,
    failed: bool,
}
impl DatabaseSaveOutcome {
    pub fn failed() -> Self {
        Self {
            edit: None,
            failed: true,
        }
    }
    pub fn is_failed(&self) -> bool {
        self.failed
    }
}
impl DatabaseSaveRequest {
    pub fn commit(self, owner: &ApplicationTasks) -> DatabaseSaveOutcome {
        match owner.application().save_database_for_application(
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
            self.set_error(
                Some(yss_ui::text::t("native.databases.saveFailed").into()),
                cx,
            );
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
        let job = self.services.run(move |_| Ok(request.commit(&owner)));
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(DatabaseSaveOutcome {
                    edit: None,
                    failed: true,
                });
            view.update_in(cx, |view, _, cx| view.finish_save(outcome, cx))
                .log_released();
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
            let receipt = services.mutate_database_for_application(
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
            view.update_in(cx, |view, window, cx| {
                view.mutating = false;
                match result {
                    Some(edit) => {
                        view.edit = Some(edit);
                        view.ready = false;
                    }
                    None => {
                        view.set_error(
                            Some(yss_ui::text::t("native.databases.columnUpdateFailed").into()),
                            cx,
                        );
                    }
                }
                if view.refresh_again {
                    view.reload(false, window, cx);
                }
                view.changed(cx);
            })
            .log_released();
        })
        .detach();
        self.changed(cx);
    }
    pub fn can_export(&self) -> bool {
        !self.busy() && self.ready
    }

    /// `format` is the file extension: `csv` or `parquet`.
    pub fn export(&mut self, format: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_export() {
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
            view.update_in(cx, |view, window, cx| {
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
            })
            .log_released();
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
            services.export_database_for_application(
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
            view.update_in(cx, |view, window, cx| {
                view.busy = false;
                if result.is_none() {
                    view.set_error(
                        Some(yss_ui::text::t("native.databases.exportFailed").into()),
                        cx,
                    );
                }
                if view.refresh_again {
                    view.reload(true, window, cx);
                }
                view.changed(cx);
            })
            .log_released();
        })
        .detach();
        self.changed(cx);
    }
}
