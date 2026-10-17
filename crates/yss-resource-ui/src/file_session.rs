//! One editing session per open authored file; the owning editor keeps its unsubmitted draft.
use gpui_kit::component::{
    Sizable,
    button::Button,
    dock::{PanelEvent, PanelInfo, PanelState},
    notification::Notification,
};
use gpui_kit::{
    Context, EventEmitter, IntoElement, ParentElement, SharedString, Styled, Task, Window,
};
use std::sync::Arc;
use yss_application::file_resources::FileApplicationError;
use yss_desktop_services::ApplicationTasks;
use yss_project::{
    ProjectOperationError,
    file_resources::{FileCommand, FileSnapshot, ResourceFile},
};
use yss_project_history::{ResourceDeltaEvent, ResourceDocumentPatch};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_model::file::{FileContent, FilePath, FileVersion};
use yss_ui::lifecycle::LogReleased;

pub enum FileEditorEvent {
    /// The editor became the target of workbench commands and inspection.
    Activated,
    /// The file no longer exists. An editor without unsaved content can close.
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFailure {
    Save,
    Read,
    Conflict,
    Removed,
}

#[derive(Default)]
enum PendingFileRefresh {
    #[default]
    None,
    Revision(ResourceRevision),
    Full,
}

impl PendingFileRefresh {
    fn accept(&mut self, delta: &ResourceDeltaEvent) {
        let ResourceDocumentPatch::ResourceLifecycle(patch) = &delta.payload else {
            *self = Self::Full;
            return;
        };
        let (Some(before), Some(after)) = (&patch.before, &patch.after) else {
            *self = Self::Full;
            return;
        };
        // Creation/deletion and path changes can replace a file's editing session.
        // Only body changes within the same resource have comparable revisions.
        if before.path != after.path
            || before.kind != after.kind
            || before.name != after.name
            || delta.to_revision <= delta.from_revision
        {
            *self = Self::Full;
            return;
        }
        match self {
            Self::None => *self = Self::Revision(delta.to_revision),
            Self::Revision(revision) => *revision = (*revision).max(delta.to_revision),
            Self::Full => {}
        }
    }

    fn take_needed(&mut self, current: ResourceRevision) -> bool {
        match std::mem::take(self) {
            Self::None => false,
            Self::Revision(revision) => revision > current,
            Self::Full => true,
        }
    }
}

pub struct FileSession<T: ResourceFile> {
    services: Arc<ApplicationTasks>,
    snapshot: FileSnapshot<T>,
    committing: bool,
    reading: bool,
    pending: PendingFileRefresh,
    conflict: Option<FileSnapshot<T>>,
    removed: bool,
    error: Option<SharedString>,
    notifications: yss_ui::notifications::Notifications,
    read_task: Option<Task<()>>,
    commit_task: Option<Task<()>>,
}

impl<T: ResourceFile> FileSession<T> {
    pub fn new(
        services: Arc<ApplicationTasks>,
        snapshot: FileSnapshot<T>,
        window: &Window,
    ) -> Self {
        Self {
            services,
            snapshot,
            committing: false,
            reading: false,
            pending: PendingFileRefresh::default(),
            conflict: None,
            removed: false,
            error: None,
            notifications: yss_ui::notifications::Notifications::new(window),
            read_task: None,
            commit_task: None,
        }
    }

    pub fn services(&self) -> &Arc<ApplicationTasks> {
        &self.services
    }

    pub fn snapshot(&self) -> &FileSnapshot<T> {
        &self.snapshot
    }

    pub fn busy(&self) -> bool {
        self.committing || self.reading
    }

    pub fn committing(&self) -> bool {
        self.committing
    }

    pub fn reading(&self) -> bool {
        self.reading
    }

    pub fn removed(&self) -> bool {
        self.removed
    }

    pub fn conflicted(&self) -> bool {
        self.conflict.is_some()
    }

    /// Editing commands need a live, current file that no other operation holds.
    pub fn editable(&self) -> bool {
        !self.busy() && !self.removed && self.conflict.is_none()
    }

    pub fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Feedback for the editor's own operations shares the session's single toast slot.
    pub fn notifications(&self) -> &yss_ui::notifications::Notifications {
        &self.notifications
    }

    pub fn report<E: 'static>(&mut self, message: impl Into<SharedString>, cx: &mut Context<E>) {
        let message = message.into();
        self.notifications.error(message.clone(), cx);
        self.error = Some(message);
    }

    /// Holds the file for a commit outside the save path, such as an immediate edit batch.
    pub fn begin_commit(&mut self) -> bool {
        if !self.editable() {
            return false;
        }
        self.committing = true;
        self.error = None;
        true
    }

    pub fn set_commit_task(&mut self, task: Task<()>) {
        self.commit_task = Some(task);
    }

    fn failure_message(&self, failure: FileFailure) -> SharedString {
        yss_ui::text::format(
            match failure {
                FileFailure::Save => "native.files.saveFailed",
                FileFailure::Read => "native.files.readFailed",
                FileFailure::Conflict => "native.files.conflict",
                FileFailure::Removed => "native.files.removed",
            },
            &[("name", self.snapshot.path.name().to_owned())],
        )
        .into()
    }
}

pub trait FileEditor:
    EventEmitter<PanelEvent> + EventEmitter<FileEditorEvent> + Sized + 'static
{
    type File: ResourceFile;
    /// The dock factory that restores this editor.
    const PANEL_NAME: &'static str;
    /// The persisted panel field holding the file path.
    const PATH_KEY: &'static str;

    fn session(&self) -> &FileSession<Self::File>;
    fn session_mut(&mut self) -> &mut FileSession<Self::File>;
    /// Changes held by the editor that the session has not submitted.
    fn has_draft(&self) -> bool;
    /// Captures unsubmitted changes when a save starts; `None` blocks the save.
    fn draft_edits(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<Vec<<Self::File as FileContent>::Edit>>;
    /// The session installed a snapshot; `keep_draft` retains the unsubmitted changes.
    fn snapshot_replaced(&mut self, keep_draft: bool, window: &mut Window, cx: &mut Context<Self>);
    /// Editors that reconcile drafts item by item accept newer snapshots without a conflict.
    fn conflicts_with_draft(&self) -> bool {
        self.has_draft()
    }
    /// A session operation finished, including after a failure.
    fn session_settled(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn session_failed(&mut self, failure: FileFailure, cx: &mut Context<Self>) {
        report_failure(self, failure, cx);
    }

    fn path(&self) -> &str {
        self.session().snapshot.path.as_str()
    }

    fn version(&self) -> &FileVersion {
        &self.session().snapshot.version
    }

    fn tab_title(&self) -> SharedString {
        let name = self.session().snapshot.path.name();
        if self.dirty() {
            format!("{name} *").into()
        } else {
            name.to_owned().into()
        }
    }

    fn panel_state(&self) -> PanelState {
        let mut state = PanelState::new(Self::PANEL_NAME);
        state.info = PanelInfo::Panel(serde_json::json!({ (Self::PATH_KEY): self.path() }));
        state
    }

    fn busy(&self) -> bool {
        self.session().busy()
    }

    /// Unsaved content, including content kept after its file was removed.
    fn unsaved(&self) -> bool {
        self.session().snapshot.dirty || self.has_draft()
    }

    /// Content that a save could still write.
    fn dirty(&self) -> bool {
        !self.session().removed && self.unsaved()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty() {
            return;
        }
        let Some(request) = self.prepare_save(cx) else {
            return;
        };
        let owner = self.session().services.clone();
        let job = self
            .session()
            .services
            .run(move |_| Ok(request.commit(&owner)));
        let task = cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_else(FileSaveOutcome::failure);
            view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx))
                .log_released();
        });
        self.session_mut().commit_task = Some(task);
    }

    fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<FileSaveRequest<Self::File>> {
        if self.session().conflicted() {
            self.session_failed(FileFailure::Conflict, cx);
            return None;
        }
        if !self.session().editable() {
            return None;
        }
        let edits = self.draft_edits(cx)?;
        let session = self.session_mut();
        session.committing = true;
        session.error = None;
        let request = FileSaveRequest {
            project: session.snapshot.project_instance_id.clone(),
            path: session.snapshot.path.clone(),
            version: session.snapshot.version.clone(),
            edits,
        };
        cx.notify();
        Some(request)
    }

    fn finish_save(
        &mut self,
        outcome: FileSaveOutcome<Self::File>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_commit(outcome.snapshot, window, cx);
        if outcome.failed {
            self.session_failed(FileFailure::Save, cx);
        }
        cx.emit(PanelEvent::LayoutChanged);
    }

    fn cancel_prepared_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_commit(None, window, cx);
    }

    /// Releases a commit; a committed snapshot replaces the session's base without dropping drafts.
    fn finish_commit(
        &mut self,
        snapshot: Option<FileSnapshot<Self::File>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.session_mut().committing = false;
        if let Some(snapshot) = snapshot {
            install(self, snapshot, true, window, cx);
        }
        settle(self, window, cx);
    }

    /// Holds a clean file while a resource operation, such as a rename, commits elsewhere.
    fn lock(&mut self, cx: &mut Context<Self>) -> bool {
        if self.dirty() || !self.session_mut().begin_commit() {
            return false;
        }
        cx.notify();
        true
    }

    fn accept_change(
        &mut self,
        delta: &ResourceDeltaEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.session_mut().pending.accept(delta);
        refresh_if_needed(self, window, cx);
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.session_mut().pending = PendingFileRefresh::Full;
        refresh_if_needed(self, window, cx);
    }

    /// A rename keeps the body, so a current snapshot moves in place without another read.
    fn accept_move(
        &mut self,
        path: FilePath<Self::File>,
        from: ResourceRevision,
        to: ResourceRevision,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = self.session_mut();
        if session.snapshot.version.revision == from && session.conflict.is_none() {
            session.snapshot.version.revision = to;
        } else {
            // The retained conflict was read at the old path; read it again at the new one.
            session.conflict = None;
            session.pending = PendingFileRefresh::Full;
        }
        session.snapshot.path = path;
        refresh_if_needed(self, window, cx);
        cx.emit(PanelEvent::LayoutChanged);
        cx.notify();
    }

    fn mark_removed(&mut self, cx: &mut Context<Self>) {
        let session = self.session_mut();
        if session.removed {
            return;
        }
        session.removed = true;
        session.conflict = None;
        session.pending = PendingFileRefresh::None;
        session.reading = false;
        session.read_task = None;
        if self.unsaved() {
            self.session_failed(FileFailure::Removed, cx);
        }
        cx.emit(FileEditorEvent::Removed);
        cx.emit(PanelEvent::LayoutChanged);
        cx.notify();
    }

    /// Reloading discards the draft; keeping it rebases the draft so the next save writes it.
    fn resolve_conflict(&mut self, keep_draft: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.session_mut().conflict.take() else {
            return;
        };
        self.session_mut().error = None;
        install(self, snapshot, keep_draft, window, cx);
        settle(self, window, cx);
        cx.emit(PanelEvent::LayoutChanged);
    }
}

/// Localization key for the file state shown by Details panels.
pub fn file_status<E: FileEditor>(editor: &E) -> &'static str {
    let session = editor.session();
    if session.removed {
        "native.files.statusRemoved"
    } else if session.conflict.is_some() {
        "native.files.statusConflict"
    } else if session.busy() {
        "native.workbench.loadingOrSaving"
    } else if editor.dirty() {
        "native.workbench.unsavedChanges"
    } else {
        "native.workbench.saved"
    }
}

pub fn report_failure<E: FileEditor>(editor: &mut E, failure: FileFailure, cx: &mut Context<E>) {
    let session = editor.session_mut();
    let message = session.failure_message(failure);
    session.error = Some(message.clone());
    if failure != FileFailure::Conflict {
        session.notifications.error(message, cx);
        return;
    }
    let owner = cx.entity().downgrade();
    session.notifications.push(
        Notification::warning(message)
            .autohide(false)
            .content(move |_, _, cx| {
                let reload = owner.clone();
                let keep = owner.clone();
                gpui_kit::div()
                    .flex()
                    .gap_2()
                    .pt_2()
                    .child(
                        Button::new("reload-file")
                            .small()
                            .label(yss_ui::text::t("native.files.reload"))
                            .on_click(cx.listener(move |toast, _, window, cx| {
                                toast.dismiss(window, cx);
                                reload
                                    .update(cx, |view, cx| view.resolve_conflict(false, window, cx))
                                    .log_released();
                            })),
                    )
                    .child(
                        Button::new("keep-file-draft")
                            .small()
                            .label(yss_ui::text::t("native.files.keepChanges"))
                            .on_click(cx.listener(move |toast, _, window, cx| {
                                toast.dismiss(window, cx);
                                keep.update(cx, |view, cx| view.resolve_conflict(true, window, cx))
                                    .log_released();
                            })),
                    )
                    .into_any_element()
            }),
        cx,
    );
}

fn install<E: FileEditor>(
    editor: &mut E,
    snapshot: FileSnapshot<E::File>,
    keep_draft: bool,
    window: &mut Window,
    cx: &mut Context<E>,
) {
    let session = editor.session_mut();
    // A receipt or read captured before a move still names the old path.
    if snapshot.path != session.snapshot.path {
        session.pending = PendingFileRefresh::Full;
        return;
    }
    session.snapshot = snapshot;
    editor.snapshot_replaced(keep_draft, window, cx);
    cx.emit(PanelEvent::LayoutChanged);
}

fn settle<E: FileEditor>(editor: &mut E, window: &mut Window, cx: &mut Context<E>) {
    refresh_if_needed(editor, window, cx);
    editor.session_settled(window, cx);
    cx.notify();
}

fn refresh_if_needed<E: FileEditor>(editor: &mut E, window: &mut Window, cx: &mut Context<E>) {
    let session = editor.session_mut();
    if session.busy() || session.removed {
        return;
    }
    let current = session
        .conflict
        .as_ref()
        .unwrap_or(&session.snapshot)
        .version
        .revision;
    if !session.pending.take_needed(current) {
        return;
    }
    session.reading = true;
    let project = session.snapshot.project_instance_id.clone();
    let path = session.snapshot.path.clone();
    let job = session
        .services
        .run(move |services| Ok(services.read_file(project, path)));
    session.read_task = Some(cx.spawn_in(window, async move |view, cx| {
        let result = job.await.ok().and_then(Result::ok);
        view.update_in(cx, |view, window, cx| finish_read(view, result, window, cx))
            .log_released();
    }));
    cx.notify();
}

fn finish_read<E: FileEditor>(
    editor: &mut E,
    result: Option<Result<FileSnapshot<E::File>, FileApplicationError>>,
    window: &mut Window,
    cx: &mut Context<E>,
) {
    if !editor.session().reading {
        return;
    }
    editor.session_mut().reading = false;
    match result {
        Some(Ok(snapshot)) if snapshot.version == editor.session().snapshot.version => {
            editor.session_mut().conflict = None;
        }
        Some(Ok(snapshot)) if snapshot.path != editor.session().snapshot.path => {
            editor.session_mut().pending = PendingFileRefresh::Full;
        }
        Some(Ok(snapshot)) if editor.conflicts_with_draft() => {
            editor.session_mut().conflict = Some(snapshot);
            editor.session_failed(FileFailure::Conflict, cx);
        }
        Some(Ok(snapshot)) => {
            editor.session_mut().error = None;
            install(editor, snapshot, false, window, cx);
        }
        Some(Err(FileApplicationError::Project(ProjectOperationError::ResourceNotFound {
            ..
        }))) => {
            editor.mark_removed(cx);
            return;
        }
        Some(Err(_)) | None => editor.session_failed(FileFailure::Read, cx),
    }
    settle(editor, window, cx);
}

pub struct FileSaveRequest<T: ResourceFile> {
    project: ProjectInstanceId,
    path: FilePath<T>,
    version: FileVersion,
    edits: Vec<T::Edit>,
}

pub struct FileSaveOutcome<T: FileContent> {
    snapshot: Option<FileSnapshot<T>>,
    failed: bool,
}

impl<T: FileContent> FileSaveOutcome<T> {
    pub fn new(snapshot: Option<FileSnapshot<T>>, failed: bool) -> Self {
        Self { snapshot, failed }
    }

    pub fn failure() -> Self {
        Self::new(None, true)
    }

    pub fn failed(&self) -> bool {
        self.failed
    }
}

impl<T: ResourceFile> FileSaveRequest<T> {
    pub fn commit(self, owner: &ApplicationTasks) -> FileSaveOutcome<T> {
        let services = owner.application();
        let mut version = self.version;
        let mut snapshot = None;
        if !self.edits.is_empty() {
            let receipt = services.apply_file_command(
                self.project.clone(),
                OperationId::new(),
                FileCommand::Edit {
                    path: self.path.clone(),
                    version: version.clone(),
                    edits: self.edits,
                },
            );
            let Ok(receipt) = receipt else {
                return FileSaveOutcome::failure();
            };
            owner.publish_resource(receipt.mutation);
            let Some(edited) = receipt.snapshot else {
                return FileSaveOutcome::failure();
            };
            version = edited.version.clone();
            snapshot = Some(edited);
        }
        match services.apply_file_command(
            self.project,
            OperationId::new(),
            FileCommand::Save {
                path: self.path,
                version,
            },
        ) {
            Ok(receipt) => {
                owner.publish_resource(receipt.mutation);
                FileSaveOutcome::new(receipt.snapshot.clone(), receipt.snapshot.is_none())
            }
            Err(_) => FileSaveOutcome::new(snapshot, true),
        }
    }
}

#[cfg(test)]
mod tests;
