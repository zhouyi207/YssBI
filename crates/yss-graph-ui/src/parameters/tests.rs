use super::*;
use crate::{GraphServices, OpenedGraph, canvas::GraphCanvas, details::GraphDetails};
use gpui_kit::component::{
    dock::BasePanel,
    input::{InputEvent, InputState},
};
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext, WindowOptions};
use std::sync::Arc;
use yss_application::{
    graph::{editing::GraphEditRequest, open::OpenGraphRequest},
    session::ApplicationState,
};
use yss_graph_document::{
    DocumentNode, GraphResourceKind, GraphResourcePath, NodeId, NodePosition,
};
use yss_graph_editor::EditorGraphMutation;
use yss_project_identity::OperationId;

struct Fixture {
    _directory: tempfile::TempDir,
    _runtime: tokio::runtime::Runtime,
    application: ApplicationState,
    window: AnyWindowHandle,
    graphs: [Entity<GraphCanvas>; 2],
    details: Entity<GraphDetails>,
    nodes: [NodeId; 2],
}

impl Fixture {
    fn new(cx: &mut TestAppContext) -> Self {
        Self::with_node_type(cx, "yssbi.dataframe.series.text.replace")
    }

    fn with_node_type(cx: &mut TestAppContext, node_type: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let nodes = [NodeId::new(), NodeId::new()];
        let paths = ["events/First.yssbi-event", "events/Second.yssbi-event"];
        let mut data = yss_project_model::ProjectData::new();
        for path in paths {
            let mut resource = yss_project_model::GraphResourceDocument::new(
                "Drafts",
                GraphResourceKind::EventGraph,
            );
            for node in nodes {
                Arc::make_mut(&mut resource.document).nodes.insert(
                    node,
                    DocumentNode {
                        id: node,
                        node_type: node_type.parse().unwrap(),
                        position: NodePosition { x: 0., y: 0. },
                        parameters: Default::default(),
                        user_label: None,
                    },
                );
            }
            data.graphs
                .insert(GraphResourcePath::new(path).unwrap(), resource);
        }
        yss_project::fixtures::write_project(&data, directory.path().to_str().unwrap()).unwrap();
        let application = ApplicationState::initialize().unwrap();
        application
            .load_project_for_application(directory.path().join("metadata.yssbi").to_str().unwrap())
            .unwrap();
        let project = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let language = yss_ui::text::locale();
        let catalog = Arc::new(
            application
                .query_project_presentation(project.clone(), language)
                .unwrap()
                .activity_panels
                .into_iter()
                .find(|panel| panel.panel_id == "nodes")
                .unwrap(),
        );
        let services = Arc::new(GraphServices::new(
            Arc::new(yss_desktop_services::ApplicationTasks::new(
                application.clone(),
                runtime.handle().clone(),
                Default::default(),
            )),
            Arc::new(yss_ui::layout_store::LayoutStore::new(
                directory.path(),
                runtime.handle().clone(),
            )),
        ));
        let opened = paths.map(|path| {
            OpenedGraph::from_open(
                application
                    .open_graph(OpenGraphRequest::new(
                        project.clone(),
                        GraphResourcePath::new(path).unwrap(),
                        0,
                        language,
                    ))
                    .unwrap(),
                language,
            )
        });
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::preferences::init(cx);
        });
        let mut views = None;
        let (window, _) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                let graphs = opened.map(|graph| {
                    cx.new(|cx| {
                        GraphCanvas::new(
                            services.clone(),
                            graph,
                            catalog.clone(),
                            language.into(),
                            window,
                            cx,
                        )
                    })
                });
                let details = cx.new(|cx| GraphDetails::new(services, window, cx));
                let root = graphs[0].clone();
                views = Some((graphs, details));
                root
            })
            .unwrap()
        });
        let (graphs, details) = views.unwrap();
        Self {
            _directory: directory,
            _runtime: runtime,
            application,
            window,
            graphs,
            details,
            nodes,
        }
    }

    fn select(&self, graph: usize, node: usize, cx: &mut TestAppContext) -> Entity<ParameterForm> {
        cx.update_window(self.window, |_, window, cx| {
            let canvas = self.graphs[graph].read(cx);
            let projection = canvas.graph().projection().clone();
            let version = canvas.graph().editing().version;
            self.details.update(cx, |details, cx| {
                details.set_selection(
                    self.graphs[graph].downgrade(),
                    vec![self.nodes[node]],
                    projection,
                    version,
                    window,
                    cx,
                )
            });
            self.graphs[graph]
                .read(cx)
                .parameter_form(self.nodes[node])
                .unwrap()
        })
        .unwrap()
    }

    fn type_text(
        &self,
        form: &Entity<ParameterForm>,
        text: &str,
        cx: &mut TestAppContext,
    ) -> ParameterKey {
        cx.update_window(self.window, |_, window, cx| {
            let field = form
                .read(cx)
                .fields
                .iter()
                .find(|field| matches!(field.draft, ParameterDraft::Text { .. }))
                .unwrap();
            let key = field.model.key.clone();
            let ParameterDraft::Text {
                input: yss_ui::input::TextField::Single(input),
                ..
            } = &field.draft
            else {
                panic!("text replacement uses single-line fields");
            };
            let input: Entity<InputState> = input.clone();
            input.update(cx, |input, cx| {
                input.set_value(text.to_owned(), window, cx);
                cx.emit(InputEvent::Change);
            });
            key
        })
        .unwrap()
    }

    fn edit_request(&self, graph: usize, cx: &mut TestAppContext) -> GraphEditRequest {
        cx.update(|cx| {
            let canvas = self.graphs[graph].read(cx);
            GraphEditRequest {
                project_instance_id: canvas.graph().project().clone(),
                graph_path: canvas.graph().projection().graph_path.clone(),
                version: canvas.graph().editing().version,
                operation_id: OperationId::new(),
                locale: yss_ui::text::locale().into(),
            }
        })
    }
}

#[gpui_kit::test]
fn pending_parameters_survive_node_and_graph_switching_and_save_atomically(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::new(cx);
    let first = fixture.select(0, 0, cx);
    let key = fixture.type_text(&first, "DRAFT_SHOULD_SURVIVE", cx);
    let second = fixture.select(0, 1, cx);
    let second_key = fixture.type_text(&second, "SECOND_NODE", cx);
    let other_graph = fixture.select(1, 0, cx);
    fixture.type_text(&other_graph, "OTHER_GRAPH", cx);
    let returned = fixture.select(0, 0, cx);
    cx.update(|cx| {
        assert_eq!(first.entity_id(), returned.entity_id());
        assert_eq!(
            first
                .read(cx)
                .fields
                .iter()
                .find(|field| field.model.key == key)
                .unwrap()
                .value(cx)
                .unwrap(),
            serde_json::json!("DRAFT_SHOULD_SURVIVE")
        );
        assert!(fixture.graphs[0].read(cx).dirty(cx));
        assert!(!fixture.graphs[0].read(cx).closable(cx));
        assert!(fixture.graphs[1].read(cx).dirty(cx));
    });
    let request = cx
        .update(|cx| fixture.graphs[0].update(cx, |canvas, cx| canvas.prepare_save(cx)))
        .unwrap();
    let outcome = request.commit(&fixture.application);
    cx.update(|cx| {
        assert!(fixture.graphs[0].update(cx, |canvas, cx| canvas.finish_command(outcome, cx)));
        assert!(!fixture.graphs[0].read(cx).dirty(cx));
        assert!(fixture.graphs[1].read(cx).dirty(cx));
    });
    let request = fixture.edit_request(0, cx);
    let document = fixture
        .application
        .current_graph_document(
            &request.project_instance_id,
            &request.graph_path,
            request.version,
        )
        .unwrap();
    assert_eq!(
        document.nodes[&fixture.nodes[0]].parameters[&key],
        serde_json::json!("DRAFT_SHOULD_SURVIVE")
    );
    assert_eq!(
        document.nodes[&fixture.nodes[1]].parameters[&second_key],
        serde_json::json!("SECOND_NODE")
    );
    let undone = fixture
        .application
        .change_graph_history(request, false)
        .unwrap();
    assert!(
        !undone.update.document.nodes[&fixture.nodes[0]]
            .parameters
            .contains_key(&key)
    );
    assert!(
        !undone.update.document.nodes[&fixture.nodes[1]]
            .parameters
            .contains_key(&second_key)
    );
}

#[gpui_kit::test]
fn stale_parameter_base_blocks_save_without_losing_text_and_restore_adopts_current_value(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::new(cx);
    let form = fixture.select(0, 0, cx);
    let key = fixture.type_text(&form, "local draft", cx);
    let response = fixture
        .application
        .edit_graph(
            fixture.edit_request(0, cx),
            EditorGraphMutation::SetParameters {
                node_id: fixture.nodes[0],
                parameters: [(key.clone(), serde_json::json!("external value"))]
                    .into_iter()
                    .collect(),
            },
        )
        .unwrap();
    cx.update(|cx| {
        fixture.graphs[0].update(cx, |canvas, _| {
            canvas
                .graph
                .install_edit(response, yss_ui::text::locale())
                .unwrap()
        })
    });
    fixture.select(0, 1, cx);
    fixture.select(0, 0, cx);
    cx.update(|cx| {
        assert!(
            fixture.graphs[0]
                .update(cx, |canvas, cx| canvas.prepare_save(cx))
                .is_none()
        );
        let field = form
            .read(cx)
            .fields
            .iter()
            .find(|field| field.model.key == key)
            .unwrap();
        assert!(field.dirty);
        assert!(field.error.is_some());
        assert_eq!(field.value(cx).unwrap(), serde_json::json!("local draft"));
    });
    cx.update_window(fixture.window, |_, window, cx| {
        form.update(cx, |form, cx| {
            let index = form
                .fields
                .iter()
                .position(|field| field.model.key == key)
                .unwrap();
            form.restore_parameter(index, window, cx);
            assert!(!form.fields[index].dirty);
            assert_eq!(
                form.fields[index].value(cx).unwrap(),
                serde_json::json!("external value")
            );
        })
    })
    .unwrap();
}

#[gpui_kit::test]
fn invalid_numeric_draft_blocks_save_and_survives_unrelated_parameter_edits(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::with_node_type(cx, "yssbi.dataframe.series.text.substring");
    let form = fixture.select(0, 0, cx);
    let key = fixture.type_text(&form, "-", cx);
    cx.update(|cx| {
        assert!(
            fixture.graphs[0]
                .update(cx, |canvas, cx| canvas.prepare_save(cx))
                .is_none()
        );
        assert!(
            form.read(cx)
                .fields
                .iter()
                .find(|field| field.model.key == key)
                .unwrap()
                .error
                .is_some()
        );
    });
    let other = cx.update(|cx| {
        form.read(cx)
            .fields
            .iter()
            .find(|field| field.model.key != key)
            .unwrap()
            .model
            .key
            .clone()
    });
    let response = fixture
        .application
        .edit_graph(
            fixture.edit_request(0, cx),
            EditorGraphMutation::SetParameters {
                node_id: fixture.nodes[0],
                parameters: [(other, serde_json::json!(5))].into_iter().collect(),
            },
        )
        .unwrap();
    cx.update(|cx| {
        fixture.graphs[0].update(cx, |canvas, _| {
            canvas
                .graph
                .install_edit(response, yss_ui::text::locale())
                .unwrap()
        })
    });
    fixture.select(0, 1, cx);
    fixture.select(0, 0, cx);
    cx.update(|cx| {
        let field = form
            .read(cx)
            .fields
            .iter()
            .find(|field| field.model.key == key)
            .unwrap();
        assert!(field.dirty);
        assert!(field.error.is_some());
        let ParameterDraft::Text { input, .. } = &field.draft else {
            panic!("numeric input");
        };
        assert_eq!(input.value(cx).as_ref(), "-");
        assert!(
            fixture.graphs[0]
                .update(cx, |canvas, cx| canvas.prepare_save(cx))
                .is_none()
        );
    });
}
