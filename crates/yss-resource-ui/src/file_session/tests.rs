use super::*;
use crate::documents::DocumentEditor;
use gpui_kit::component::dock::PanelInfo;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext, WindowOptions};
use std::{cell::Cell, path::PathBuf, rc::Rc};
use yss_application::{
    events::{CommittedResourceMutation, ResourceRoute},
    file_resources::FileMutation,
    session::ApplicationState,
};
use yss_desktop_services::DesktopEvents;
use yss_project::docs::DocSnapshot;
use yss_project_history::{
    DocResourceKey, ResourceKey, ResourceLifecycleKind, ResourceLifecyclePatch,
    ResourceLifecycleState,
};
use yss_project_model::{
    ProjectData,
    doc::{DocDocument, DocEdit},
};

fn changed(from: u64, to: u64) -> ResourceDeltaEvent {
    let state = |revision| ResourceLifecycleState {
        revision: ResourceRevision::new(revision),
        path: "docs/notes.md".into(),
        name: "notes".into(),
        kind: ResourceLifecycleKind::Doc,
    };
    ResourceDeltaEvent {
        resource: ResourceKey::Doc(DocResourceKey("docs/notes.md".into())),
        from_revision: ResourceRevision::new(from),
        to_revision: ResourceRevision::new(to),
        caused_by: None,
        payload: ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
            before: Some(state(from)),
            after: Some(state(to)),
        }),
    }
}

#[test]
fn save_receipt_covers_events_delivered_before_or_after_completion() {
    let mut pending = PendingFileRefresh::default();
    pending.accept(&changed(0, 1));
    pending.accept(&changed(1, 2));
    assert!(!pending.take_needed(ResourceRevision::new(2)));

    pending.accept(&changed(1, 2));
    pending.accept(&changed(0, 1));
    assert!(!pending.take_needed(ResourceRevision::new(2)));
}

#[test]
fn newer_external_change_survives_older_save_completion_and_events() {
    let mut pending = PendingFileRefresh::default();
    pending.accept(&changed(2, 3));
    pending.accept(&changed(1, 2));
    assert!(pending.take_needed(ResourceRevision::new(2)));
    assert!(!pending.take_needed(ResourceRevision::new(3)));
}

#[test]
fn incomplete_save_receipt_does_not_discard_a_committed_change() {
    let mut pending = PendingFileRefresh::default();
    pending.accept(&changed(1, 2));
    assert!(pending.take_needed(ResourceRevision::new(1)));
}

#[test]
fn unversioned_invalidation_cannot_be_satisfied_by_a_save_receipt() {
    let mut pending = PendingFileRefresh::Full;
    pending.accept(&changed(1, 2));
    assert!(pending.take_needed(ResourceRevision::new(2)));
}

#[test]
fn recreated_resource_is_read_even_when_its_revision_restarts() {
    let mut delta = changed(0, 0);
    if let ResourceDocumentPatch::ResourceLifecycle(patch) = &mut delta.payload {
        patch.before = None;
    }
    let mut pending = PendingFileRefresh::default();
    pending.accept(&delta);
    pending.accept(&changed(0, 1));
    assert!(pending.take_needed(ResourceRevision::new(10)));
}

/// A real project behind the editor, changed by "other writers" through the same Application
/// commands the Assistant uses.
struct Project {
    root: PathBuf,
    application: ApplicationState,
    identity: ProjectInstanceId,
    tasks: Arc<ApplicationTasks>,
    _runtime: tokio::runtime::Runtime,
    _directory: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        yss_project::fixtures::write_project(
            &ProjectData::new(),
            directory.path().to_str().unwrap(),
        )
        .unwrap();
        let application = ApplicationState::initialize().unwrap();
        application
            .load_project_for_application(directory.path().join("metadata.yssbi").to_str().unwrap())
            .unwrap();
        let identity = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let tasks = Arc::new(ApplicationTasks::new(
            application.clone(),
            runtime.handle().clone(),
            DesktopEvents::default(),
        ));
        Self {
            root: directory.path().to_owned(),
            application,
            identity,
            tasks,
            _runtime: runtime,
            _directory: directory,
        }
    }

    fn command(&self, command: FileCommand<DocDocument>) -> FileMutation<DocDocument> {
        self.application
            .apply_file_command(self.identity.clone(), OperationId::new(), command)
            .unwrap()
    }

    fn create(&self, name: &str) -> DocSnapshot {
        self.command(FileCommand::Create { name: name.into() })
            .snapshot
            .unwrap()
    }

    fn disk(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(path)).ok()
    }
}

fn open(
    project: &Project,
    snapshot: DocSnapshot,
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, Entity<DocumentEditor>) {
    // Commits use real Tokio workers; do not jump GPUI's virtual clock past their work.
    cx.executor().allow_parking();
    let tasks = project.tasks.clone();
    cx.update(|cx| {
        gpui_kit::init(cx);
        yss_ui::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| DocumentEditor::new(tasks, snapshot, window, cx))
        })
        .unwrap()
    })
}

fn type_text(
    window: AnyWindowHandle,
    document: &Entity<DocumentEditor>,
    text: &str,
    cx: &mut TestAppContext,
) {
    cx.update_window(window, |_, window, cx| {
        document.update(cx, |document, cx| document.focus_editor(window, cx));
        window.render_frame(cx);
        window.input(text, cx);
    })
    .unwrap();
}

/// Delivers a published mutation the way the workbench routes it to one open editor.
fn deliver(
    window: AnyWindowHandle,
    document: &Entity<DocumentEditor>,
    mutation: &CommittedResourceMutation,
    cx: &mut TestAppContext,
) {
    cx.update_window(window, |_, window, cx| {
        document.update(cx, |document, cx| {
            for route in mutation.routes() {
                match route {
                    ResourceRoute::Moved {
                        to,
                        from_revision,
                        to_revision,
                        ..
                    } => document.accept_move(
                        FilePath::parse(to).unwrap(),
                        from_revision,
                        to_revision,
                        window,
                        cx,
                    ),
                    ResourceRoute::Removed { .. } => document.mark_removed(cx),
                    ResourceRoute::Changed { delta, .. } => {
                        document.accept_change(delta, window, cx)
                    }
                }
            }
        })
    })
    .unwrap();
}

fn save(window: AnyWindowHandle, document: &Entity<DocumentEditor>, cx: &mut TestAppContext) {
    cx.update_window(window, |_, window, cx| {
        document.update(cx, |document, cx| document.save(window, cx))
    })
    .unwrap();
}

#[gpui_kit::test]
async fn a_rename_elsewhere_moves_the_open_draft_and_its_next_save(cx: &mut TestAppContext) {
    let project = Project::new();
    let created = project.create("Notes");
    let (window, document) = open(&project, created.clone(), cx);
    type_text(window, &document, "moved draft", cx);

    let renamed = project.command(FileCommand::Rename {
        path: created.path,
        version: created.version,
        name: "Renamed".into(),
    });
    assert!(matches!(
        renamed.mutation.routes().as_slice(),
        [ResourceRoute::Moved {
            from: "docs/Notes.md",
            to: "docs/Renamed.md",
            ..
        }]
    ));
    deliver(window, &document, &renamed.mutation, cx);
    cx.update(|cx| {
        let document = document.read(cx);
        assert_eq!(document.path(), "docs/Renamed.md");
        assert!(document.dirty(), "the move keeps the unsaved draft");
        assert!(
            !document.busy(),
            "a current snapshot moves without another read"
        );
        assert_eq!(document.tab_title().as_ref(), "Renamed *");
        let state = document.panel_state();
        assert_eq!(state.panel_name, "document-editor");
        assert!(matches!(
            &state.info,
            PanelInfo::Panel(info) if *info == serde_json::json!({"documentPath": "docs/Renamed.md"})
        ));
    });

    save(window, &document, cx);
    cx.condition(&document, |document, _| !document.busy())
        .await;
    cx.update(|cx| assert!(!document.read(cx).dirty()));
    assert_eq!(
        project.disk("docs/Renamed.md").as_deref(),
        Some("moved draft")
    );
    assert_eq!(project.disk("docs/Notes.md"), None);
}

#[gpui_kit::test]
async fn a_newer_version_waits_for_the_draft_owner_to_keep_or_reload(cx: &mut TestAppContext) {
    let project = Project::new();
    let created = project.create("Notes");
    let (window, document) = open(&project, created.clone(), cx);
    type_text(window, &document, "mine", cx);

    let theirs = project.command(FileCommand::Edit {
        path: created.path.clone(),
        version: created.version,
        edits: vec![DocEdit::SetMarkdown {
            markdown: "theirs".into(),
        }],
    });
    let edited = theirs.snapshot.clone().unwrap();
    let saved = project.command(FileCommand::Save {
        path: edited.path,
        version: edited.version,
    });
    deliver(window, &document, &theirs.mutation, cx);
    deliver(window, &document, &saved.mutation, cx);
    cx.condition(&document, |document, _| {
        document.session().conflicted() && !document.busy()
    })
    .await;
    cx.update(|cx| {
        document.update(cx, |document, cx| {
            assert!(document.dirty());
            assert!(
                document.prepare_save(cx).is_none(),
                "a conflicted draft must not save over the newer version"
            );
        })
    });

    cx.update_window(window, |_, window, cx| {
        document.update(cx, |document, cx| {
            document.resolve_conflict(true, window, cx)
        })
    })
    .unwrap();
    save(window, &document, cx);
    cx.condition(&document, |document, _| !document.busy())
        .await;
    cx.update(|cx| assert!(!document.read(cx).dirty()));
    assert_eq!(project.disk("docs/Notes.md").as_deref(), Some("mine"));

    let current = cx.update(|cx| document.read(cx).snapshot().clone());
    type_text(window, &document, " again", cx);
    let replaced = project.command(FileCommand::Edit {
        path: current.path.clone(),
        version: current.version,
        edits: vec![DocEdit::SetMarkdown {
            markdown: "replacement".into(),
        }],
    });
    deliver(window, &document, &replaced.mutation, cx);
    cx.condition(&document, |document, _| {
        document.session().conflicted() && !document.busy()
    })
    .await;
    cx.update_window(window, |_, window, cx| {
        document.update(cx, |document, cx| {
            document.resolve_conflict(false, window, cx)
        })
    })
    .unwrap();
    cx.update(|cx| {
        let document = document.read(cx);
        assert!(!document.session().conflicted());
        assert_eq!(document.snapshot().content.0, "replacement");
        assert!(
            !document.has_draft(),
            "reloading replaces the draft with the newer version"
        );
    });
}

#[gpui_kit::test]
async fn deletion_closes_clean_editors_and_detaches_unsaved_drafts(cx: &mut TestAppContext) {
    let project = Project::new();
    let clean = project.create("Clean");
    let (window, document) = open(&project, clean.clone(), cx);
    let removed = Rc::new(Cell::new(false));
    cx.update(|cx| {
        let removed = removed.clone();
        cx.subscribe(&document, move |_, event: &FileEditorEvent, _| {
            if matches!(event, FileEditorEvent::Removed) {
                removed.set(true);
            }
        })
        .detach();
    });
    let deleted = project.command(FileCommand::Delete {
        path: clean.path,
        version: clean.version,
    });
    deliver(window, &document, &deleted.mutation, cx);
    cx.run_until_parked();
    assert!(removed.get());
    cx.update(|cx| assert!(!document.read(cx).unsaved(), "a clean editor can close"));

    let drafted = project.create("Draft");
    let (window, document) = open(&project, drafted.clone(), cx);
    type_text(window, &document, "kept", cx);
    let deleted = project.command(FileCommand::Delete {
        path: drafted.path,
        version: drafted.version,
    });
    deliver(window, &document, &deleted.mutation, cx);
    cx.update(|cx| {
        let document = document.read(cx);
        assert!(
            document.unsaved(),
            "the draft stays visible after its file is gone"
        );
        assert!(!document.dirty(), "a removed file is not a save target");
        assert!(!document.session().editable());
    });
    save(window, &document, cx);
    cx.update(|cx| assert!(!document.read(cx).busy()));
    assert_eq!(project.disk("docs/Draft.md"), None);
}

#[gpui_kit::test]
async fn an_external_deletion_of_a_clean_file_reaches_the_editor_as_removal(
    cx: &mut TestAppContext,
) {
    let project = Project::new();
    let created = project.create("External");
    let (window, document) = open(&project, created, cx);
    std::fs::remove_file(project.root.join("docs/External.md")).unwrap();
    project
        .application
        .reconcile_project_change(
            &project.identity,
            yss_filesystem::change::FilesystemChange::rescan_required(),
        )
        .unwrap();
    cx.update_window(window, |_, window, cx| {
        document.update(cx, |document, cx| document.refresh(window, cx))
    })
    .unwrap();
    cx.condition(&document, |document, _| document.session().removed())
        .await;
    cx.update(|cx| assert!(!document.read(cx).unsaved()));
}
