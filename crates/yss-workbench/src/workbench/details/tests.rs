use super::super::files::WeakFilePanel;
use super::{DetailsPanel, DetailsTarget};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Context, Entity, IntoElement, Render, Subscription,
    TestAppContext, Window, WindowOptions, div, prelude::*,
};
use std::{cell::Cell, rc::Rc, sync::Arc};
use yss_application::session::{ApplicationSessionSlot, ApplicationState, NodeComponents};
use yss_data_ui::databases::DatabaseEditor;
use yss_desktop_services::{ApplicationTasks, DesktopEvents};
use yss_graph_ui::GraphServices;
use yss_project::docs::DocSnapshot;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_project_model::{
    doc::{DocDocument, DocPath},
    file::{FileContent, FileVersion},
};
use yss_resource_ui::documents::{DocumentEditor, DocumentSaveOutcome};
use yss_resource_ui::file_session::FileEditor;

// Compose the production entities; assertions observe actual GPUI notifications, not a
// second implementation of the routing/equality rules.
struct InspectorWindow {
    document: Entity<DocumentEditor>,
    other_document: Entity<DocumentEditor>,
    database: Entity<DatabaseEditor>,
    details: Entity<DetailsPanel>,
    notifications: Rc<Cell<usize>>,
    _observation: Subscription,
    _runtime: tokio::runtime::Runtime,
}

impl Render for InspectorWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(div().flex_1().h_full().child(self.document.clone()))
            .child(div().w_80().h_full().child(self.details.clone()))
    }
}

fn snapshot(name: &str) -> DocSnapshot {
    DocSnapshot {
        project_instance_id: ProjectInstanceId::new(),
        path: DocPath::parse(&format!(
            "{}/{name}.{}",
            DocDocument::DIRECTORY,
            DocDocument::EXTENSION
        ))
        .unwrap(),
        version: FileVersion {
            session_id: "inspector-test".into(),
            revision: ResourceRevision::INITIAL,
        },
        kind: DocDocument::KIND,
        content: DocDocument(String::new()),
        dirty: false,
    }
}

fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<InspectorWindow>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        yss_ui::init(cx);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(
            NodeComponents::builtins().unwrap(),
        )));
        let tasks = Arc::new(ApplicationTasks::new(
            application,
            runtime.handle().clone(),
            DesktopEvents::default(),
        ));
        let graph = Arc::new(GraphServices::new(
            tasks.clone(),
            Arc::new(yss_ui::layout_store::LayoutStore::new(
                &std::env::temp_dir(),
                runtime.handle().clone(),
            )),
        ));
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let document = cx
                    .new(|cx| DocumentEditor::new(tasks.clone(), snapshot("current"), window, cx));
                let other_document = cx.new(|cx| {
                    DocumentEditor::new(tasks.clone(), snapshot("background"), window, cx)
                });
                let database = cx.new(|cx| {
                    DatabaseEditor::new(
                        tasks,
                        ProjectInstanceId::new(),
                        "data".into(),
                        ResourceRevision::INITIAL,
                        "Data".into(),
                        window,
                        cx,
                    )
                });
                let details = cx.new(|cx| DetailsPanel::new(graph, window, cx));
                details.update(cx, |details, cx| {
                    details.set_target(
                        DetailsTarget::File(WeakFilePanel::Document(document.downgrade())),
                        cx,
                    )
                });
                let notifications = Rc::new(Cell::new(0));
                let count = notifications.clone();
                let observation = cx.observe(&details, move |_, _, _| count.set(count.get() + 1));
                InspectorWindow {
                    document,
                    other_document,
                    database,
                    details,
                    notifications,
                    _observation: observation,
                    _runtime: runtime,
                }
            })
        })
        .unwrap()
    })
}

fn notifications(view: &Entity<InspectorWindow>, cx: &mut TestAppContext) -> usize {
    cx.update(|cx| view.read(cx).notifications.replace(0))
}

#[gpui_kit::test]
fn document_input_notifies_details_only_for_visible_status_transitions(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    cx.update_window(window, |_, window, cx| {
        let document = view.read(cx).document.clone();
        document.update(cx, |document, cx| document.focus_editor(window, cx));
        window.render_frame(cx);
    })
    .unwrap();
    notifications(&view, cx);

    cx.update_window(window, |_, window, cx| window.input("a", cx))
        .unwrap();
    assert!(
        notifications(&view, cx) > 0,
        "first dirty transition must update the inspector"
    );
    cx.update_window(window, |_, window, cx| window.input("bcdef", cx))
        .unwrap();
    assert_eq!(
        notifications(&view, cx),
        0,
        "already-dirty text is not inspector state"
    );

    cx.update(|cx| {
        let document = view.read(cx).document.clone();
        assert!(document.read(cx).dirty());
        document.update(cx, |document, cx| {
            assert!(document.prepare_save(cx).is_some());
        });
    });
    assert!(
        notifications(&view, cx) > 0,
        "saving must update the inspector"
    );
    cx.update_window(window, |_, window, cx| {
        let document = view.read(cx).document.clone();
        let mut saved = document.read(cx).snapshot().clone();
        saved.content = DocDocument("abcdef".into());
        saved.version.revision = ResourceRevision::new(1);
        document.update(cx, |document, cx| {
            document.finish_save(DocumentSaveOutcome::new(Some(saved), false), window, cx)
        });
        assert!(!document.read(cx).dirty());
    })
    .unwrap();
    assert!(
        notifications(&view, cx) > 0,
        "save-clear must update the inspector"
    );

    cx.update_window(window, |_, window, cx| {
        let document = view.read(cx).document.clone();
        document.update(cx, |document, cx| {
            document.finish_save(DocumentSaveOutcome::new(None, true), window, cx)
        });
    })
    .unwrap();
    assert!(
        notifications(&view, cx) > 0,
        "an error must update an otherwise unchanged inspector"
    );
    cx.update_window(window, |_, window, cx| window.input("g", cx))
        .unwrap();
    assert!(notifications(&view, cx) > 0);
    cx.update_window(window, |_, window, cx| window.input("h", cx))
        .unwrap();
    assert_eq!(
        notifications(&view, cx),
        0,
        "retained error text does not turn input into inspector changes"
    );
}

#[gpui_kit::test]
fn document_snapshot_identity_is_an_inspector_dependency(cx: &mut TestAppContext) {
    let (window, view) = open(cx);
    notifications(&view, cx);
    cx.update_window(window, |_, window, cx| {
        let document = view.read(cx).document.clone();
        let renamed = snapshot("renamed").path;
        document.update(cx, |document, cx| {
            document.accept_move(
                renamed,
                ResourceRevision::INITIAL,
                ResourceRevision::new(1),
                window,
                cx,
            )
        });
        assert_eq!(document.read(cx).snapshot().path.name(), "renamed");
        assert!(!document.read(cx).busy());
        assert!(!document.read(cx).dirty());
    })
    .unwrap();
    assert!(
        notifications(&view, cx) > 0,
        "changed displayed path/title must invalidate otherwise unchanged details"
    );
}

#[gpui_kit::test]
fn replacing_current_target_unsubscribes_background_editors(cx: &mut TestAppContext) {
    let (_, view) = open(cx);
    notifications(&view, cx);
    cx.update(|cx| {
        let background = view.read(cx).other_document.clone();
        background.update(cx, |document, cx| {
            document.prepare_save(cx);
        });
    });
    assert_eq!(notifications(&view, cx), 0);

    cx.update(|cx| {
        let target = view.read(cx).other_document.clone();
        let details = view.read(cx).details.clone();
        details.update(cx, |details, cx| {
            details.set_target(
                DetailsTarget::File(WeakFilePanel::Document(target.downgrade())),
                cx,
            )
        });
    });
    assert!(notifications(&view, cx) > 0);
    cx.update(|cx| {
        let old_target = view.read(cx).document.clone();
        old_target.update(cx, |document, cx| {
            document.prepare_save(cx);
        });
    });
    assert_eq!(
        notifications(&view, cx),
        0,
        "the replaced target must have no live inspector subscription"
    );

    cx.update(|cx| {
        let target = view.read(cx).other_document.clone();
        let details = view.read(cx).details.clone();
        details.update(cx, |details, cx| {
            details.set_target(
                DetailsTarget::File(WeakFilePanel::Document(target.downgrade())),
                cx,
            )
        });
    });
    assert_eq!(
        notifications(&view, cx),
        0,
        "same-target activation must retain the inspector"
    );
}

#[gpui_kit::test]
fn database_errors_and_overlay_return_follow_current_target(cx: &mut TestAppContext) {
    let (_, view) = open(cx);
    cx.update(|cx| {
        let database = view.read(cx).database.clone();
        let details = view.read(cx).details.clone();
        details.update(cx, |details, cx| {
            details.set_target(DetailsTarget::Database(database.downgrade()), cx)
        });
    });
    notifications(&view, cx);
    cx.update(|cx| {
        let database = view.read(cx).database.clone();
        database.update(cx, |database, cx| database.fail_read(cx));
    });
    assert!(notifications(&view, cx) > 0);
    cx.update(|cx| {
        let database = view.read(cx).database.clone();
        database.update(cx, |database, cx| database.fail_read(cx));
    });
    assert_eq!(
        notifications(&view, cx),
        0,
        "unchanged errors must not invalidate details"
    );
    cx.update(|cx| {
        let details = view.read(cx).details.clone();
        details.update(cx, |details, cx| {
            details.show_node_definition(
                ProjectInstanceId::new(),
                yss_node_protocol::NodeTypeId::new("yssbi.value.int64").unwrap(),
                "static:yssbi.value.int64".into(),
                cx,
            )
        });
        assert!(details.read(cx).graph.read(cx).showing_node_definition());
        assert!(details.read(cx).graph_visible(cx));
        assert!(!details.read(cx).target_visible(cx));
        assert_eq!(
            details.read(cx).inspected_node(cx),
            Some("static:yssbi.value.int64")
        );
    });
    notifications(&view, cx);
    cx.update(|cx| {
        let database = view.read(cx).database.clone();
        database.update(cx, |database, cx| database.unavailable(cx));
    });
    assert_eq!(
        notifications(&view, cx),
        0,
        "a covered target must not invalidate the overlay"
    );
    cx.update(|cx| {
        let database = view.read(cx).database.clone();
        let details = view.read(cx).details.clone();
        details.update(cx, |details, cx| {
            details.set_target(DetailsTarget::Database(database.downgrade()), cx)
        });
        assert!(!details.read(cx).graph.read(cx).showing_node_definition());
        assert!(!details.read(cx).graph_visible(cx));
        assert!(details.read(cx).target_visible(cx));
        assert_eq!(details.read(cx).target_id(), Some(database.entity_id()));
        // The node catalog stops marking an entry once its definition is covered.
        assert_eq!(details.read(cx).inspected_node(cx), None);
    });
    assert!(
        notifications(&view, cx) > 0,
        "same-target activation must dismiss a covering definition"
    );
}

#[gpui_kit::test]
fn document_save_echoes_do_not_reenter_loading_or_block_following_input(cx: &mut TestAppContext) {
    use yss_project_history::{
        DocResourceKey, ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey,
        ResourceLifecycleKind, ResourceLifecyclePatch, ResourceLifecycleState,
    };

    let (window, view) = open(cx);
    cx.update_window(window, |_, window, cx| {
        let document = view.read(cx).document.clone();
        document.update(cx, |document, cx| document.focus_editor(window, cx));
        window.render_frame(cx);
        window.input("saved", cx);
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        let document = view.read(cx).document.clone();
        assert!(document.read(cx).dirty());
        document.update(cx, |document, cx| {
            assert!(document.prepare_save(cx).is_some());
            let mut saved = document.snapshot().clone();
            saved.content = DocDocument("saved".into());
            saved.version.revision = ResourceRevision::new(2);
            let state = |revision| ResourceLifecycleState {
                revision: ResourceRevision::new(revision),
                path: saved.path.as_str().into(),
                name: saved.path.name().into(),
                kind: ResourceLifecycleKind::Doc,
            };
            let delta = ResourceDeltaEvent {
                resource: ResourceKey::Doc(DocResourceKey(saved.path.as_str().into())),
                from_revision: ResourceRevision::new(1),
                to_revision: ResourceRevision::new(2),
                caused_by: None,
                payload: ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                    before: Some(state(1)),
                    after: Some(state(2)),
                }),
            };
            document.accept_change(&delta, window, cx);
            document.finish_save(DocumentSaveOutcome::new(Some(saved), false), window, cx);
            assert!(
                !document.busy(),
                "the save receipt already contains the queued revision"
            );
            assert!(!document.dirty());
            document.accept_change(&delta, window, cx);
            assert!(
                !document.busy(),
                "a delayed echo must not make the editor readonly"
            );
        });
        window.render_frame(cx);
        window.input(" next", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let document = view.read(cx).document.read(cx);
        assert!(document.dirty(), "input must remain usable after saving");
        assert_eq!(document.snapshot().content.0, "saved");
    });
}
