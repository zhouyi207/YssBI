//! Graphs and authored files share one management flow. Kinds differ only in what blocks the
//! command, its wording, the committed command and how the receipt reaches their editors.
use super::super::files::{FileKind, OpenedFile};
use super::super::name_form::NameForm;
use super::super::{Workbench, files::FilePanel};
use super::ResourceAction;
use super::names::ResourcePrompts;
use gpui_kit::{App, ClipboardItem, Context, Entity, Window};
use yss_application::events::CommittedResourceMutation;
use yss_desktop_services::ApplicationTasks;
use yss_graph_document::GraphResourcePath;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_model::file::FileVersion;
use yss_ui::lifecycle::LogReleased;

#[derive(Clone)]
enum ManagedResource {
    Graph(GraphResourcePath),
    File {
        kind: FileKind,
        /// The open editor's session when the command was captured.
        version: Option<FileVersion>,
    },
}

/// A management command captured against the version the user saw.
#[derive(Clone)]
pub(in crate::workbench) struct ResourceTarget {
    resource: ManagedResource,
    pub(in crate::workbench) project: ProjectInstanceId,
    lifecycle: u64,
    pub(in crate::workbench) path: String,
    name: String,
    pub(in crate::workbench) revision: ResourceRevision,
}

enum ManagedOutcome {
    /// The committed graph mutation and the path of the copy a duplicate created.
    Graph(CommittedResourceMutation, Option<String>),
    /// The file at its new path or the copy, when the receipt carried one.
    File(Option<OpenedFile>),
}

impl ResourceTarget {
    fn prompts(&self) -> ResourcePrompts {
        match &self.resource {
            ManagedResource::Graph(_) => ResourcePrompts {
                rename_title: crate::text::translate("native.workbench.renameGraph").into(),
                delete_title: crate::text::format(
                    "native.workbench.deleteResourceTitle",
                    &[("value0", self.name.clone())],
                )
                .into(),
                delete_message: crate::text::translate("native.workbench.deleteGraphMessage")
                    .into(),
            },
            ManagedResource::File { kind, .. } => kind.prompts(&self.name),
        }
    }

    fn failure(&self) -> &'static str {
        match self.resource {
            ManagedResource::Graph(_) => "native.workbench.graphResourceFailed",
            ManagedResource::File { .. } => "native.workbench.fileResourceFailed",
        }
    }

    fn commit(
        &self,
        owner: &ApplicationTasks,
        action: ResourceAction,
        name: Option<String>,
    ) -> anyhow::Result<ManagedOutcome> {
        let path = match &self.resource {
            ManagedResource::File { kind, version } => {
                return Ok(ManagedOutcome::File(OpenedFile::manage(
                    owner,
                    self,
                    *kind,
                    version.as_ref(),
                    action,
                    name,
                )?));
            }
            ManagedResource::Graph(path) => path.clone(),
        };
        let application = owner.application();
        let project = self.project.clone();
        let receipt = match action {
            ResourceAction::Rename => application.rename_graph_resource(
                project,
                path,
                self.revision,
                name.unwrap_or_default(),
                0,
                OperationId::new(),
            )?,
            ResourceAction::Duplicate => application.duplicate_graph_resource(
                project,
                path,
                self.revision,
                OperationId::new(),
                None,
            )?,
            ResourceAction::Delete => application.remove_graph_resource(
                project,
                path,
                self.revision,
                OperationId::new(),
            )?,
            ResourceAction::CopyPath => anyhow::bail!("copying a path does not mutate the graph"),
        };
        let copy = matches!(action, ResourceAction::Duplicate)
            .then(|| super::created_path(&receipt))
            .flatten();
        owner.publish_resource(receipt.clone());
        Ok(ManagedOutcome::Graph(receipt, copy))
    }
}

impl Workbench {
    pub(in crate::workbench) fn resource_action(
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
        let Some((name, mut revision)) = project.indexed_resource(&path) else {
            return;
        };
        let resource = if let Some(kind) = FileKind::of(&path) {
            let version = self.file_panel(&path).map(|panel| panel.version(cx));
            if let Some(version) = &version {
                revision = version.revision;
            }
            ManagedResource::File { kind, version }
        } else {
            let Ok(graph) = GraphResourcePath::new(path.as_str()) else {
                return;
            };
            if let Some(canvas) = self
                .graphs
                .get(&path)
                .and_then(gpui_kit::WeakEntity::upgrade)
            {
                revision = canvas.read(cx).resource_revision();
            }
            ManagedResource::Graph(graph)
        };
        let target = ResourceTarget {
            resource,
            project: project.identity.clone(),
            lifecycle: self.lifecycle,
            path,
            name,
            revision,
        };
        if matches!(action, ResourceAction::CopyPath) {
            cx.write_to_clipboard(ClipboardItem::new_string(target.path));
            return;
        }
        if let Some(blocked) = self.management_blocked(&target, action, cx) {
            self.error = Some(crate::text::translate(blocked));
            cx.notify();
            return;
        }
        let prompts = target.prompts();
        self.confirm_resource_action(
            target.name.clone(),
            action,
            prompts,
            window,
            cx,
            move |view, name, form, window, cx| {
                view.mutate_resource(target.clone(), action, name, form, window, cx)
            },
        );
    }

    /// A running graph cannot move or disappear. A file draft lives only in its editor,
    /// so it must be saved first; a graph draft moves with its backend session.
    fn management_blocked(
        &self,
        target: &ResourceTarget,
        action: ResourceAction,
        cx: &App,
    ) -> Option<&'static str> {
        match &target.resource {
            ManagedResource::Graph(_) => {
                (matches!(action, ResourceAction::Rename | ResourceAction::Delete)
                    && self
                        .graphs
                        .get(&target.path)
                        .and_then(gpui_kit::WeakEntity::upgrade)
                        .is_some_and(|graph| graph.read(cx).is_running()))
                .then_some("native.workbench.stopGraphBeforeEditing")
            }
            ManagedResource::File { .. } => self
                .file_panel(&target.path)
                .is_some_and(|panel| panel.dirty(cx))
                .then_some("native.workbench.saveFileBeforeManaging"),
        }
    }

    /// An open file editor that changed while the dialog was open invalidates the command;
    /// otherwise it stays readonly until the receipt and its events settle.
    fn lock_file_editor(
        &mut self,
        target: &ResourceTarget,
        cx: &mut Context<Self>,
    ) -> Result<Option<FilePanel>, ()> {
        let ManagedResource::File { version, .. } = &target.resource else {
            return Ok(None);
        };
        let Some(panel) = self.file_panel(&target.path) else {
            return Ok(None);
        };
        let current = panel.version(cx);
        if panel.dirty(cx)
            || current.revision != target.revision
            || version
                .as_ref()
                .is_some_and(|original| *original != current)
        {
            self.error = Some(crate::text::translate("native.workbench.resourceChanged"));
            cx.notify();
            return Err(());
        }
        if !panel.lock(cx) {
            return Err(());
        }
        Ok(Some(panel))
    }

    fn mutate_resource(
        &mut self,
        target: ResourceTarget,
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
        if let Some(blocked) = self.management_blocked(&target, action, cx) {
            self.error = Some(crate::text::translate(blocked));
            cx.notify();
            return;
        }
        let Ok(locked) = self.lock_file_editor(&target, cx) else {
            return;
        };
        if let ManagedResource::Graph(_) = target.resource {
            // Pending canvas input must not apply to the graph while it moves or disappears.
            window.focus(&self.focus, cx);
        }
        self.busy = true;
        self.error = None;
        let request = target.clone();
        let owner = self.services.tasks.clone();
        let job = self
            .services
            .run(move |_| request.commit(&owner, action, name));
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            view.update_in(cx, |view, window, cx| {
                if let Some(panel) = &locked {
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
                    result.is_err().then_some(target.failure()),
                    window,
                    cx,
                );
                match result {
                    Ok(ManagedOutcome::Graph(receipt, copy)) => {
                        view.accept_resource_mutation(&receipt, window, cx);
                        if let Some(path) = copy {
                            view.open_graph(path, window, cx);
                        }
                    }
                    Ok(ManagedOutcome::File(Some(opened)))
                        if matches!(action, ResourceAction::Duplicate) =>
                    {
                        if let Some(panel) = view.install_file(opened, window, cx) {
                            panel.focus(window, cx);
                        }
                    }
                    // The published mutation already moved or retired the open editor.
                    Ok(ManagedOutcome::File(Some(opened))) => {
                        if let Some(panel) = view.file_panel(opened.path()) {
                            panel.focus(window, cx);
                        }
                    }
                    Ok(ManagedOutcome::File(None)) => {}
                    // A failed operation may still have written files without returning a receipt.
                    Err(_) => view.refresh_project(window, cx),
                }
                view.persist_layout(cx);
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_application::session::ApplicationState;
    use yss_desktop_services::{DesktopEvent, DesktopEvents};
    use yss_project::file_resources::FileCommand;
    use yss_project_model::doc::DocDocument;

    #[test]
    fn graph_and_file_commands_publish_their_receipt_and_report_what_opens_next() {
        let directory = tempfile::tempdir().unwrap();
        yss_project::fixtures::write_project(
            &yss_project_model::ProjectData::new(),
            directory.path().to_str().unwrap(),
        )
        .unwrap();
        let application = ApplicationState::initialize().unwrap();
        application
            .load_project_for_application(directory.path().join("metadata.yssbi").to_str().unwrap())
            .unwrap();
        let project = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let created = application
            .create_event_graph(project.clone(), "Flow".into(), OperationId::new())
            .unwrap();
        let graph = super::super::created_path(&created).unwrap();
        let document = application
            .apply_file_command::<DocDocument>(
                project.clone(),
                OperationId::new(),
                FileCommand::Create {
                    name: "Notes".into(),
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let graph_revision = application
            .query_project_index(project.clone())
            .unwrap()
            .event_graphs
            .iter()
            .find(|entry| entry.path == graph)
            .unwrap()
            .revision;
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let events = DesktopEvents::default();
        let mut published = events.subscribe();
        let owner = ApplicationTasks::new(application, runtime.handle().clone(), events);

        let duplicated = ResourceTarget {
            resource: ManagedResource::Graph(GraphResourcePath::new(graph.as_str()).unwrap()),
            project: project.clone(),
            lifecycle: 0,
            path: graph.clone(),
            name: "Flow".into(),
            revision: graph_revision,
        }
        .commit(&owner, ResourceAction::Duplicate, None);
        let Ok(ManagedOutcome::Graph(receipt, Some(copy))) = duplicated else {
            panic!("a graph duplicate reports the copy to open");
        };
        assert_ne!(copy, graph);
        assert!(
            matches!(published.try_recv(), Ok(DesktopEvent::Resource(event)) if event == receipt)
        );

        let renamed = ResourceTarget {
            resource: ManagedResource::File {
                kind: FileKind::Document,
                version: None,
            },
            project,
            lifecycle: 0,
            path: document.path.as_str().to_owned(),
            name: "Notes".into(),
            revision: document.version.revision,
        }
        .commit(&owner, ResourceAction::Rename, Some("Renamed".into()));
        let Ok(ManagedOutcome::File(Some(opened))) = renamed else {
            panic!("a file rename reports the file at its new path");
        };
        assert_eq!(opened.path(), "docs/Renamed.md");
        assert!(matches!(
            published.try_recv(),
            Ok(DesktopEvent::Resource(_))
        ));
    }
}
