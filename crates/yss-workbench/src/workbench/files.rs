//! Authored-file editors share one registry, open path, management flow and event route.
use super::Workbench;
use super::name_form::NameForm;
use super::resources::{ResourceAction, names::ResourcePrompts};
use crate::charts::ChartEditor;
use crate::documents::{DocumentEditor, DocumentSaveOutcome, DocumentSaveRequest};
use crate::file_session::{
    FileEditor, FileEditorEvent, FileRoute, file_resource_path, file_routes,
};
use crate::minds::{MindCanvas, MindSaveOutcome, MindSaveRequest};
use gpui_kit::component::dock::{BasePanelView, DockPlacement, panel_handle};
use gpui_kit::{
    AnyView, App, AppContext, ClipboardItem, Context, Entity, EntityId, WeakEntity, Window,
};
use std::sync::Arc;
use yss_application::{events::CommittedResourceMutation, session::ApplicationState};
use yss_chart_document::ChartDocument;
use yss_desktop_services::ApplicationTasks;
use yss_project::file_resources::{FileCommand, FileSnapshot, ResourceFile};
use yss_project_history::{ResourceDeltaEvent, ResourceKey};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_model::{
    doc::DocDocument,
    file::{FilePath, FileVersion},
    mind::MindDocument,
};
use yss_results_ui::charts::{ChartSaveOutcome, ChartSaveRequest};
use yss_ui::lifecycle::LogReleased;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::workbench) enum FileKind {
    Document,
    Mind,
    Chart,
}

impl FileKind {
    pub(in crate::workbench) const ALL: [Self; 3] = [Self::Document, Self::Mind, Self::Chart];

    pub(in crate::workbench) fn of(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| match kind {
            Self::Document => FilePath::<DocDocument>::parse(path).is_ok(),
            Self::Mind => FilePath::<MindDocument>::parse(path).is_ok(),
            Self::Chart => FilePath::<ChartDocument>::parse(path).is_ok(),
        })
    }

    fn category(self) -> &'static str {
        match self {
            Self::Document => "project.docs",
            Self::Mind => "project.minds",
            Self::Chart => "project.charts",
        }
    }

    /// The panel factory name and path key written by each editor's `BasePanel::dump`.
    pub(in crate::workbench) fn layout(self) -> (&'static str, &'static str) {
        match self {
            Self::Document => ("document-editor", "documentPath"),
            Self::Mind => ("mind-editor", "mindPath"),
            Self::Chart => ("chart-editor", "chartPath"),
        }
    }

    fn creation_text(self) -> (&'static str, &'static str) {
        match self {
            Self::Document => ("native.workbench.newMarkdown", "documents.newDoc"),
            Self::Mind => ("documents.newMind", "documents.newMind"),
            Self::Chart => ("menubar.newChart", "menubar.newChart"),
        }
    }

    fn prompts(self, name: &str) -> ResourcePrompts {
        let (rename_title, delete_title, delete_message) = match self {
            Self::Document | Self::Mind => (
                crate::text::translate("contextMenu.sidebar.rename"),
                crate::text::translate("documents.delete"),
                crate::text::format("documents.deleteMessage", &[("name", name.to_owned())]),
            ),
            Self::Chart => (
                crate::text::translate("contextMenu.dialog.renameChartTitle"),
                crate::text::format(
                    "native.workbench.deleteResourceTitle",
                    &[("value0", name.to_owned())],
                ),
                crate::text::translate("native.workbench.deleteChartMessage"),
            ),
        };
        ResourcePrompts {
            rename_title: rename_title.into(),
            delete_title: delete_title.into(),
            delete_message: delete_message.into(),
        }
    }

    fn open_failed(self) -> &'static str {
        match self {
            Self::Document => "native.workbench.documentOpenFailed",
            Self::Mind => "native.workbench.mindOpenFailed",
            Self::Chart => "native.workbench.chartOpenFailed",
        }
    }
}

/// A file read on a worker, ready to become an editor on the UI thread.
pub(in crate::workbench) enum OpenedFile {
    Document(FileSnapshot<DocDocument>),
    Mind(FileSnapshot<MindDocument>),
    Chart(FileSnapshot<ChartDocument>),
}

impl OpenedFile {
    pub(in crate::workbench) fn read(
        application: &ApplicationState,
        project: ProjectInstanceId,
        path: &str,
    ) -> anyhow::Result<Self> {
        fn read<T: ResourceFile>(
            application: &ApplicationState,
            project: ProjectInstanceId,
            path: &str,
        ) -> anyhow::Result<FileSnapshot<T>> {
            let path = FilePath::parse(path).map_err(anyhow::Error::msg)?;
            Ok(application.read_file(project, path)?)
        }
        Ok(
            match FileKind::of(path).ok_or_else(|| anyhow::anyhow!("not an authored file"))? {
                FileKind::Document => Self::Document(read(application, project, path)?),
                FileKind::Mind => Self::Mind(read(application, project, path)?),
                FileKind::Chart => Self::Chart(read(application, project, path)?),
            },
        )
    }

    fn create(
        owner: &ApplicationTasks,
        project: ProjectInstanceId,
        kind: FileKind,
        name: String,
    ) -> anyhow::Result<Option<Self>> {
        Ok(match kind {
            FileKind::Document => {
                commit::<DocDocument>(owner, project, FileCommand::Create { name })?
                    .map(Self::Document)
            }
            FileKind::Mind => commit::<MindDocument>(owner, project, FileCommand::Create { name })?
                .map(Self::Mind),
            FileKind::Chart => {
                commit::<ChartDocument>(owner, project, FileCommand::Create { name })?
                    .map(Self::Chart)
            }
        })
    }

    fn manage(
        owner: &ApplicationTasks,
        target: &FileTarget,
        action: ResourceAction,
        name: Option<String>,
    ) -> anyhow::Result<Option<Self>> {
        Ok(match target.kind {
            FileKind::Document => {
                manage::<DocDocument>(owner, target, action, name)?.map(Self::Document)
            }
            FileKind::Mind => manage::<MindDocument>(owner, target, action, name)?.map(Self::Mind),
            FileKind::Chart => {
                manage::<ChartDocument>(owner, target, action, name)?.map(Self::Chart)
            }
        })
    }

    fn path(&self) -> &str {
        match self {
            Self::Document(snapshot) => snapshot.path.as_str(),
            Self::Mind(snapshot) => snapshot.path.as_str(),
            Self::Chart(snapshot) => snapshot.path.as_str(),
        }
    }
}

fn commit<T: ResourceFile>(
    owner: &ApplicationTasks,
    project: ProjectInstanceId,
    command: FileCommand<T>,
) -> anyhow::Result<Option<FileSnapshot<T>>> {
    let receipt = owner
        .application()
        .apply_file_command(project, OperationId::new(), command)?;
    owner.publish_resource(receipt.mutation);
    Ok(receipt.snapshot)
}

fn manage<T: ResourceFile>(
    owner: &ApplicationTasks,
    target: &FileTarget,
    action: ResourceAction,
    name: Option<String>,
) -> anyhow::Result<Option<FileSnapshot<T>>> {
    let path = FilePath::<T>::parse(&target.path).map_err(anyhow::Error::msg)?;
    let version = match &target.version {
        Some(version) => version.clone(),
        None => {
            let snapshot = owner
                .application()
                .read_file(target.project.clone(), path.clone())?;
            anyhow::ensure!(
                snapshot.version.revision == target.revision,
                "file version changed"
            );
            snapshot.version
        }
    };
    let command = match action {
        ResourceAction::Rename => FileCommand::Rename {
            path,
            version,
            name: name.ok_or_else(|| anyhow::anyhow!("file name missing"))?,
        },
        ResourceAction::Duplicate => FileCommand::Duplicate {
            path,
            version,
            name: None,
        },
        ResourceAction::Delete => FileCommand::Delete { path, version },
        ResourceAction::CopyPath => anyhow::bail!("copying a path does not mutate the file"),
    };
    commit(owner, target.project.clone(), command)
}

/// A retained authored-file editor. The workbench routes its lifecycle without reading its draft.
#[derive(Clone)]
pub(crate) enum FilePanel {
    Document(Entity<DocumentEditor>),
    Mind(Entity<MindCanvas>),
    Chart(Entity<ChartEditor>),
}

#[derive(Clone)]
pub(crate) enum WeakFilePanel {
    Document(WeakEntity<DocumentEditor>),
    Mind(WeakEntity<MindCanvas>),
    Chart(WeakEntity<ChartEditor>),
}

impl WeakFilePanel {
    pub(crate) fn upgrade(&self) -> Option<FilePanel> {
        Some(match self {
            Self::Document(editor) => FilePanel::Document(editor.upgrade()?),
            Self::Mind(editor) => FilePanel::Mind(editor.upgrade()?),
            Self::Chart(editor) => FilePanel::Chart(editor.upgrade()?),
        })
    }
}

macro_rules! with_editor {
    ($panel:expr, $editor:ident => $body:expr) => {
        match $panel {
            FilePanel::Document($editor) => $body,
            FilePanel::Mind($editor) => $body,
            FilePanel::Chart($editor) => $body,
        }
    };
}

pub(in crate::workbench) enum FileSave {
    Document(DocumentSaveRequest),
    Mind(MindSaveRequest),
    Chart(ChartSaveRequest),
}

pub(in crate::workbench) enum FileSaved {
    Document(DocumentSaveOutcome),
    Mind(MindSaveOutcome),
    Chart(ChartSaveOutcome),
}

impl FileSave {
    pub(in crate::workbench) fn commit(self, owner: &ApplicationTasks) -> FileSaved {
        match self {
            Self::Document(request) => FileSaved::Document(request.commit(owner)),
            Self::Mind(request) => FileSaved::Mind(request.commit(owner)),
            Self::Chart(request) => FileSaved::Chart(request.commit(owner)),
        }
    }
}

impl FilePanel {
    pub(crate) fn from_view(view: AnyView) -> Option<Self> {
        view.clone()
            .downcast::<DocumentEditor>()
            .map(Self::Document)
            .or_else(|_| view.clone().downcast::<MindCanvas>().map(Self::Mind))
            .or_else(|_| view.downcast::<ChartEditor>().map(Self::Chart))
            .ok()
    }

    pub(crate) fn downgrade(&self) -> WeakFilePanel {
        match self {
            Self::Document(editor) => WeakFilePanel::Document(editor.downgrade()),
            Self::Mind(editor) => WeakFilePanel::Mind(editor.downgrade()),
            Self::Chart(editor) => WeakFilePanel::Chart(editor.downgrade()),
        }
    }

    pub(crate) fn handle(&self) -> Arc<dyn BasePanelView> {
        with_editor!(self, editor => panel_handle(editor.clone()))
    }

    pub(crate) fn entity_id(&self) -> EntityId {
        with_editor!(self, editor => editor.entity_id())
    }

    pub(crate) fn dirty(&self, cx: &App) -> bool {
        with_editor!(self, editor => editor.read(cx).dirty())
    }

    pub(crate) fn unsaved(&self, cx: &App) -> bool {
        with_editor!(self, editor => editor.read(cx).unsaved())
    }

    pub(crate) fn busy(&self, cx: &App) -> bool {
        with_editor!(self, editor => editor.read(cx).busy())
    }

    fn path(&self, cx: &App) -> String {
        with_editor!(self, editor => editor.read(cx).path().to_owned())
    }

    fn version(&self, cx: &App) -> FileVersion {
        with_editor!(self, editor => editor.read(cx).version().clone())
    }

    /// Workbench status that changes command availability, not every edit.
    fn status(&self, cx: &App) -> (bool, bool, bool) {
        let editing = match self {
            Self::Document(editor) => editor.read(cx).is_editing(),
            Self::Mind(_) | Self::Chart(_) => false,
        };
        (self.dirty(cx), self.busy(cx), editing)
    }

    pub(crate) fn save(&self, window: &mut Window, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| editor.save(window, cx)))
    }

    pub(crate) fn refresh(&self, window: &mut Window, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| editor.refresh(window, cx)))
    }

    fn accept_change(&self, delta: &ResourceDeltaEvent, window: &mut Window, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| {
            editor.accept_change(delta, window, cx)
        }))
    }

    fn accept_move(
        &self,
        path: &str,
        from: ResourceRevision,
        to: ResourceRevision,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        with_editor!(self, editor => {
            let Ok(path) = FilePath::parse(path) else {
                return false;
            };
            editor.update(cx, |editor, cx| editor.accept_move(path, from, to, window, cx));
            true
        })
    }

    fn mark_removed(&self, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| editor.mark_removed(cx)))
    }

    fn lock(&self, cx: &mut App) -> bool {
        with_editor!(self, editor => editor.update(cx, |editor, cx| editor.lock(cx)))
    }

    fn unlock(&self, window: &mut Window, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| {
            editor.finish_commit(None, window, cx)
        }))
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Document(editor) => {
                editor.update(cx, |editor, cx| editor.focus_editor(window, cx))
            }
            Self::Mind(editor) => editor.update(cx, |editor, cx| editor.focus_canvas(window, cx)),
            Self::Chart(editor) => editor.update(cx, |editor, cx| editor.focus_chart(window, cx)),
        }
    }

    pub(in crate::workbench) fn prepare_save(&self, cx: &mut App) -> Option<FileSave> {
        match self {
            Self::Document(editor) => editor
                .update(cx, |editor, cx| editor.prepare_save(cx))
                .map(FileSave::Document),
            Self::Mind(editor) => editor
                .update(cx, |editor, cx| editor.prepare_save(cx))
                .map(FileSave::Mind),
            Self::Chart(editor) => editor
                .update(cx, |editor, cx| editor.prepare_save(cx))
                .map(FileSave::Chart),
        }
    }

    /// Installs a save receipt and reports whether the save failed.
    pub(in crate::workbench) fn finish_save(
        &self,
        outcome: FileSaved,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        match (self, outcome) {
            (Self::Document(editor), FileSaved::Document(outcome)) => {
                let failed = outcome.failed();
                editor.update(cx, |editor, cx| editor.finish_save(outcome, window, cx));
                failed
            }
            (Self::Mind(editor), FileSaved::Mind(outcome)) => {
                let failed = outcome.failed();
                editor.update(cx, |editor, cx| editor.finish_save(outcome, window, cx));
                failed
            }
            (Self::Chart(editor), FileSaved::Chart(outcome)) => {
                let failed = outcome.failed();
                editor.update(cx, |editor, cx| editor.finish_save(outcome, window, cx));
                failed
            }
            _ => {
                self.fail_save(window, cx);
                true
            }
        }
    }

    pub(in crate::workbench) fn fail_save(&self, window: &mut Window, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| {
            editor.finish_save(crate::file_session::FileSaveOutcome::failure(), window, cx)
        }))
    }

    pub(in crate::workbench) fn cancel_save(&self, window: &mut Window, cx: &mut App) {
        with_editor!(self, editor => editor.update(cx, |editor, cx| {
            editor.cancel_prepared_save(window, cx)
        }))
    }
}

#[derive(Clone)]
struct FileTarget {
    kind: FileKind,
    project: ProjectInstanceId,
    lifecycle: u64,
    path: String,
    name: String,
    revision: ResourceRevision,
    version: Option<FileVersion>,
}

impl Workbench {
    pub(in crate::workbench) fn file_panel(&self, path: &str) -> Option<FilePanel> {
        self.files.get(path).and_then(WeakFilePanel::upgrade)
    }

    pub(in crate::workbench) fn file_panels(&self) -> impl Iterator<Item = FilePanel> + '_ {
        self.files.values().filter_map(WeakFilePanel::upgrade)
    }

    pub(in crate::workbench) fn open_file(
        &mut self,
        path: String,
        intent: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        if self.busy || self.closing {
            return;
        }
        let Some(kind) = FileKind::of(&path) else {
            if let Some(intent) = intent {
                self.finish_intent(&intent, false, window, cx);
            }
            return;
        };
        if let Some(panel) = self.file_panel(&path) {
            self.present_file(&panel, window, cx);
            panel.focus(window, cx);
            if let Some(intent) = intent {
                self.finish_intent(&intent, true, window, cx);
            }
            return;
        }
        if !self.opening.insert(path.clone()) {
            if let Some(intent) = intent {
                self.finish_intent(&intent, false, window, cx);
            }
            return;
        }
        let identity = project.identity.clone();
        let expected = identity.clone();
        let lifecycle = self.lifecycle;
        let query_path = path.clone();
        let task = self
            .services
            .run(move |services| OpenedFile::read(&services.application, identity, &query_path));
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != expected)
                {
                    return;
                }
                view.opening.remove(&path);
                if intent
                    .as_deref()
                    .is_some_and(|id| !view.is_current_intent(id))
                {
                    return;
                }
                let installed = result.and_then(|opened| view.install_file(opened, window, cx));
                let applied = if let Some(panel) = installed {
                    panel.focus(window, cx);
                    true
                } else {
                    view.error = Some(crate::text::t(kind.open_failed()).into());
                    false
                };
                if let Some(intent) = intent {
                    view.finish_intent(&intent, applied, window, cx);
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
    }

    fn present_file(&self, panel: &FilePanel, window: &mut Window, cx: &mut Context<Self>) {
        self.present_panel(panel.handle(), DockPlacement::Center, window, cx);
    }

    pub(in crate::workbench) fn install_file(
        &mut self,
        opened: OpenedFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<FilePanel> {
        let catalog = self.project.as_ref()?.index.clone();
        let path = opened.path().to_owned();
        let services = self.services.tasks.clone();
        let panel = match opened {
            OpenedFile::Document(snapshot) => FilePanel::Document(
                cx.new(|cx| DocumentEditor::new(services, snapshot, window, cx)),
            ),
            OpenedFile::Mind(snapshot) => {
                FilePanel::Mind(cx.new(|cx| MindCanvas::new(services, snapshot, window, cx)))
            }
            OpenedFile::Chart(snapshot) => FilePanel::Chart(
                cx.new(|cx| ChartEditor::new(services, snapshot, catalog.clone(), window, cx)),
            ),
        };
        let mut status = panel.status(cx);
        let observed = panel.downgrade();
        with_editor!(&panel, editor => {
            self.subscriptions.push(cx.observe(editor, move |_, _, cx| {
                let Some(observed) = observed.upgrade() else {
                    return;
                };
                let next = observed.status(cx);
                if status != next {
                    status = next;
                    cx.notify();
                }
            }));
            let target = panel.downgrade();
            self.subscriptions.push(cx.subscribe_in(
                editor,
                window,
                move |view, _, event, window, cx| {
                    let Some(panel) = target.upgrade() else {
                        return;
                    };
                    match event {
                        FileEditorEvent::Activated => view.activate_file_panel(&panel, cx),
                        FileEditorEvent::Removed => {
                            if !panel.unsaved(cx) {
                                view.retire_file(&panel, window, cx);
                            }
                        }
                    }
                },
            ));
        });
        self.files.insert(path, panel.downgrade());
        self.present_file(&panel, window, cx);
        if let FilePanel::Chart(chart) = &panel {
            // The first catalog delivery starts the preview.
            chart.update(cx, |chart, cx| chart.replace_catalog(catalog, window, cx));
        }
        Some(panel)
    }

    fn activate_file_panel(&mut self, panel: &FilePanel, cx: &mut Context<Self>) {
        self.active_editor = Some(panel.entity_id().into());
        let target = panel.downgrade();
        self.details
            .update(cx, |details, cx| details.set_file(target, cx));
        let path = panel.path(cx);
        self.activate_file(path, cx);
    }

    pub(in crate::workbench) fn retire_file(
        &mut self,
        panel: &FilePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.files.retain(|_, retained| {
            retained
                .upgrade()
                .is_none_or(|retained| retained.entity_id() != panel.entity_id())
        });
        if self
            .details
            .read(cx)
            .file()
            .is_some_and(|current| current.entity_id() == panel.entity_id())
        {
            self.details.update(cx, |details, cx| details.clear(cx));
        }
        with_editor!(panel, editor => self
            .dock
            .update(cx, |dock, cx| dock.remove_panel(editor.clone(), window, cx)));
        self.persist_layout(cx);
    }

    pub(in crate::workbench) fn refresh_files(&self, window: &mut Window, cx: &mut Context<Self>) {
        for panel in self.file_panels().collect::<Vec<_>>() {
            panel.refresh(window, cx);
        }
    }

    pub(in crate::workbench) fn refresh_file_resources(
        &self,
        resources: &[ResourceKey],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for path in resources.iter().filter_map(file_resource_path) {
            if let Some(panel) = self.file_panel(path) {
                panel.refresh(window, cx);
            }
        }
    }

    pub(in crate::workbench) fn update_chart_catalogs(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        for panel in self.file_panels().collect::<Vec<_>>() {
            if let FilePanel::Chart(chart) = panel {
                let catalog = project.index.clone();
                chart.update(cx, |chart, cx| chart.replace_catalog(catalog, window, cx));
            }
        }
    }

    /// The single route for committed file changes, whether the GUI, the Assistant or
    /// another writer made them: moves follow the editor, deletions retire or detach it.
    pub(in crate::workbench) fn accept_file_mutation(
        &mut self,
        mutation: &CommittedResourceMutation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut moved_any = false;
        for route in file_routes(mutation) {
            match route {
                FileRoute::Moved {
                    from,
                    to,
                    from_revision,
                    to_revision,
                } => {
                    let Some(panel) = self.file_panel(from) else {
                        continue;
                    };
                    // A destination editor left by an earlier deletion keeps its own draft.
                    if self.file_panel(to).is_some()
                        || !panel.accept_move(to, from_revision, to_revision, window, cx)
                    {
                        panel.refresh(window, cx);
                        continue;
                    }
                    self.files.remove(from);
                    self.files.insert(to.to_owned(), panel.downgrade());
                    moved_any = true;
                }
                FileRoute::Removed { path } => {
                    if let Some(panel) = self.file_panel(path) {
                        panel.mark_removed(cx);
                    }
                }
                FileRoute::Changed { path, delta } => {
                    if let Some(panel) = self.file_panel(path) {
                        panel.accept_change(delta, window, cx);
                    }
                }
            }
        }
        if moved_any {
            self.persist_layout(cx);
        }
    }

    pub(in crate::workbench) fn create_file_dialog(
        &mut self,
        kind: FileKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &self.project else {
            return;
        };
        if self.busy || self.closing {
            return;
        }
        let project = project.identity.clone();
        let (title, default_name) = kind.creation_text();
        self.resource_name_dialog(
            crate::text::t(title),
            crate::text::t(default_name).into(),
            crate::text::translate("contextMenu.dialog.createSubmit"),
            window,
            cx,
            move |view, name, form, window, cx| {
                view.create_file(kind, project.clone(), name, form, window, cx);
            },
        );
    }

    fn create_file(
        &mut self,
        kind: FileKind,
        project: ProjectInstanceId,
        name: String,
        form: Entity<NameForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.error = None;
        let lifecycle = self.lifecycle;
        let expected = project.clone();
        let owner = self.services.tasks.clone();
        let job = self
            .services
            .run(move |_| OpenedFile::create(&owner, project, kind, name));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.lifecycle != lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|current| current.identity != expected)
                {
                    NameForm::fail(Some(&form), "native.workbench.resourceChanged", cx);
                    return;
                }
                view.busy = false;
                view.finish_resource_name(
                    Some(form),
                    result
                        .is_none()
                        .then_some("native.workbench.fileCreateFailed"),
                    window,
                    cx,
                );
                match result {
                    Some(Some(opened)) => {
                        view.expand_project_category(kind.category(), cx);
                        if let Some(panel) = view.install_file(opened, window, cx) {
                            panel.focus(window, cx);
                        }
                    }
                    Some(None) => {
                        view.expand_project_category(kind.category(), cx);
                        view.error = Some(crate::text::translate(
                            "native.workbench.resourceCommittedUnavailable",
                        ));
                    }
                    // Successful commits publish a resource event, which owns the index refresh.
                    // A failed operation may still have written files without returning a receipt.
                    None => view.refresh_project(window, cx),
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    pub(in crate::workbench) fn file_resource_action(
        &mut self,
        path: String,
        action: ResourceAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || self.opening.contains(&path) {
            return;
        }
        let Some(project) = &self.project else {
            return;
        };
        let Some(kind) = FileKind::of(&path) else {
            return;
        };
        let Some((name, revision)) = project.indexed_file(&path) else {
            return;
        };
        let panel = self.file_panel(&path);
        let version = panel.as_ref().map(|panel| panel.version(cx));
        let target = FileTarget {
            kind,
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
            path,
            name,
            revision: version
                .as_ref()
                .map_or(revision, |version| version.revision),
            version,
        };
        if matches!(action, ResourceAction::CopyPath) {
            cx.write_to_clipboard(ClipboardItem::new_string(target.path));
            return;
        }
        if panel.as_ref().is_some_and(|panel| panel.dirty(cx)) {
            self.error = Some(crate::text::translate(
                "native.workbench.saveFileBeforeManaging",
            ));
            cx.notify();
            return;
        }
        let prompts = kind.prompts(&target.name);
        self.confirm_resource_action(
            target.name.clone(),
            action,
            prompts,
            window,
            cx,
            move |view, name, form, window, cx| {
                view.mutate_file(target.clone(), action, name, form, window, cx)
            },
        );
    }

    fn mutate_file(
        &mut self,
        target: FileTarget,
        action: ResourceAction,
        name: Option<String>,
        form: Option<Entity<NameForm>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx)
            || self.lifecycle != target.lifecycle
            || self.opening.contains(&target.path)
            || self
                .project
                .as_ref()
                .is_none_or(|project| project.identity != target.project)
        {
            return;
        }
        let panel = self.file_panel(&target.path);
        if let Some(panel) = &panel {
            let version = panel.version(cx);
            if panel.dirty(cx)
                || version.revision != target.revision
                || target
                    .version
                    .as_ref()
                    .is_some_and(|original| *original != version)
            {
                self.error = Some(crate::text::translate("native.workbench.resourceChanged"));
                cx.notify();
                return;
            }
            // The editor stays readonly until the operation's receipt and events settle.
            if !panel.lock(cx) {
                return;
            }
        }
        self.busy = true;
        self.error = None;
        let request = target.clone();
        let owner = self.services.tasks.clone();
        let job = self
            .services
            .run(move |_| OpenedFile::manage(&owner, &request, action, name));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if let Some(panel) = &panel {
                    panel.unlock(window, cx);
                }
                if view.lifecycle != target.lifecycle
                    || view
                        .project
                        .as_ref()
                        .is_none_or(|project| project.identity != target.project)
                {
                    NameForm::fail(form.as_ref(), "native.workbench.resourceChanged", cx);
                    return;
                }
                view.busy = false;
                view.finish_resource_name(
                    form,
                    result
                        .is_none()
                        .then_some("native.workbench.fileResourceFailed"),
                    window,
                    cx,
                );
                match result {
                    // The published mutation already moved or retired the open editor.
                    Some(Some(opened)) if matches!(action, ResourceAction::Duplicate) => {
                        if let Some(panel) = view.install_file(opened, window, cx) {
                            panel.focus(window, cx);
                        }
                    }
                    Some(Some(opened)) => {
                        if let Some(panel) = view.file_panel(opened.path()) {
                            panel.focus(window, cx);
                        }
                    }
                    Some(None) => {}
                    None => view.refresh_project(window, cx),
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }
}
