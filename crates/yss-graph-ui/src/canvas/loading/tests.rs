use super::*;
use crate::{GraphServices, OpenedGraph};
use gpui_kit::{AppContext, MouseButton, MouseDownEvent, TestAppContext, WindowOptions, point, px};
use std::sync::Arc;
use yss_application::{
    graph::{editing::GraphEditRequest, open::OpenGraphRequest},
    session::ApplicationState,
};
use yss_graph_document::{DocumentNode, GraphResourceKind, NodeId, NodePosition};
use yss_graph_editor::EditorGraphMutation;
use yss_project_history::{
    DatabaseResourceKey, DocResourceKey, FunctionResourceKey, MindResourceKey, ResourceKey,
};
use yss_project_identity::OperationId;

#[test]
fn own_change_events_do_not_refresh_before_or_after_the_command_receipt() {
    let mut delivery = ChangeDelivery::default();
    let edit = OperationId::new();
    let save = OperationId::new();
    assert!(!delivery.receive(edit, true));
    assert!(!delivery.finish(&[edit, save]));
    assert!(!delivery.receive(save, false));
}

#[test]
fn unrelated_changes_survive_acknowledging_a_successful_command_prefix() {
    let mut delivery = ChangeDelivery::default();
    let edit = OperationId::new();
    let external = OperationId::new();
    assert!(!delivery.receive(edit, true));
    assert!(!delivery.receive(external, true));
    assert!(delivery.finish(&[edit]));
    assert!(!delivery.finish(&[]));
    assert!(delivery.receive(OperationId::new(), false));
}

#[test]
fn cancelled_preparation_keeps_external_invalidation() {
    let mut delivery = ChangeDelivery::default();
    assert!(!delivery.receive(OperationId::new(), true));
    assert!(delivery.finish(&[]));
}

struct Fixture {
    directory: tempfile::TempDir,
    application: ApplicationState,
    request: OpenGraphRequest,
    source: NodeId,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = GraphResourcePath::new("events/Refresh.yssbi-event").unwrap();
        let mut data = yss_project_model::ProjectData::new();
        let mut resource =
            yss_project_model::GraphResourceDocument::new("Refresh", GraphResourceKind::EventGraph);
        let source = NodeId::new();
        for (id, kind, parameter, value) in [
            (
                source,
                "yssbi.dataframe.source.get",
                "dataframe",
                "missing-data",
            ),
            (
                NodeId::new(),
                "yssbi.project.function.call",
                "target",
                "functions/Missing.yssbi-function",
            ),
        ] {
            Arc::make_mut(&mut resource.document).nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: kind.parse().unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: [(parameter.parse().unwrap(), serde_json::json!(value))].into(),
                    user_label: None,
                },
            );
        }
        data.graphs.insert(path.clone(), resource);
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
        let request = OpenGraphRequest::new(project, path, yss_ui::text::locale());
        Self {
            directory,
            application,
            request,
            source,
        }
    }

    fn open(&self) -> OpenedGraph {
        OpenedGraph::from_open(
            self.application.open_graph(self.request.clone()).unwrap(),
            self.request.locale(),
        )
    }

    fn edit(
        &self,
        graph: &OpenedGraph,
        mutation: EditorGraphMutation,
    ) -> yss_application::graph::editing::GraphEditResponse {
        self.application
            .edit_graph(
                GraphEditRequest {
                    project_instance_id: graph.project().clone(),
                    graph_path: graph.projection().graph_path.clone(),
                    version: graph.editing().version,
                    operation_id: OperationId::new(),
                    locale: graph.language().to_owned(),
                },
                mutation,
            )
            .unwrap()
    }
}

#[test]
fn routing_uses_observed_missing_dependencies_and_tracks_edits() {
    let fixture = Fixture::new();
    let mut graph = fixture.open();
    let database = |id: &str| ResourceKey::Database(DatabaseResourceKey(id.into()));
    assert!(graph.depends_on_resource(&database("missing-data")));
    assert!(!graph.depends_on_resource(&database("unrelated")));
    assert!(
        graph.depends_on_resource(&ResourceKey::Function(FunctionResourceKey(
            "functions/Missing.yssbi-function".into()
        )))
    );
    assert!(!graph.depends_on_resource(&ResourceKey::Doc(DocResourceKey("docs/Notes.md".into()))));
    assert!(
        !graph.depends_on_resource(&ResourceKey::Mind(MindResourceKey(
            "minds/Notes.yssbi-mind".into()
        )))
    );
    let edited = fixture.edit(
        &graph,
        EditorGraphMutation::DeleteNodes {
            node_ids: vec![fixture.source],
        },
    );
    graph
        .install_edit(edited, fixture.request.locale())
        .unwrap();
    assert!(!graph.depends_on_resource(&database("missing-data")));
    assert!(
        graph.depends_on_resource(&ResourceKey::Function(FunctionResourceKey(
            "functions/Missing.yssbi-function".into()
        )))
    );
}

#[gpui_kit::test]
fn unchanged_refresh_preserves_gesture_and_projection_but_real_edit_replaces_them(
    cx: &mut TestAppContext,
) {
    let fixture = Fixture::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    let same = fixture.open();
    cx.update_window(window, |_, window, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.begin_pan(
                None,
                &MouseDownEvent {
                    button: MouseButton::Middle,
                    position: point(px(40.), px(40.)),
                    ..Default::default()
                },
                window,
                cx,
            );
            let projection = canvas.graph.projection.clone();
            let presentation = canvas.presentation.clone();
            let details = canvas.port_details.clone();
            canvas.install_projection(same, cx);
            assert!(
                canvas.gesture.is_some(),
                "an equal read cancelled the active pan"
            );
            assert!(Arc::ptr_eq(&projection, &canvas.graph.projection));
            assert!(std::rc::Rc::ptr_eq(&presentation, &canvas.presentation));
            assert!(
                details
                    .as_ref()
                    .zip(canvas.port_details.as_ref())
                    .is_some_and(|(before, after)| std::rc::Rc::ptr_eq(before, after))
            );
            let edited = fixture.edit(
                &canvas.graph,
                EditorGraphMutation::SetNodeLabel {
                    node_id: fixture.source,
                    label: Some("Changed source".into()),
                },
            );
            let changed = fixture.open();
            assert_eq!(changed.editing().version, edited.editing.version);
            canvas.install_projection(changed, cx);
            assert!(canvas.gesture.is_none());
            assert!(!Arc::ptr_eq(&projection, &canvas.graph.projection));
            assert!(
                canvas
                    .graph
                    .projection
                    .nodes
                    .iter()
                    .any(|node| node.display.user_label.as_deref() == Some("Changed source"))
            );
        });
    })
    .unwrap();
}

#[test]
fn refresh_identity_includes_locale_runtime_and_resolver_basis() {
    let fixture = Fixture::new();
    let original = fixture.open();
    let mut incoming = fixture.open();
    assert!(original.same_projection(&incoming));
    incoming.language = "another-language".into();
    assert!(!original.same_projection(&incoming));
    incoming.language = original.language.clone();
    incoming.results.execution_session_id =
        yss_graph_execution::identity::ExecutionSessionId::new(uuid::Uuid::new_v4());
    assert!(!original.same_projection(&incoming));
    incoming.results.execution_session_id = original.results.execution_session_id;
    Arc::make_mut(&mut incoming.projection)
        .basis
        .registry_fingerprint = [42; 32];
    assert!(!original.same_projection(&incoming));
}

impl Fixture {
    fn canvas(
        &self,
        runtime: &tokio::runtime::Runtime,
        cx: &mut TestAppContext,
    ) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<GraphCanvas>) {
        // Commands use real Tokio workers; do not jump GPUI's virtual clock past their work.
        cx.executor().allow_parking();
        let tasks = Arc::new(yss_desktop_services::ApplicationTasks::new(
            self.application.clone(),
            runtime.handle().clone(),
            Default::default(),
        ));
        let services = Arc::new(GraphServices::new(
            tasks,
            Arc::new(yss_ui::layout_store::LayoutStore::new(
                self.directory.path(),
                runtime.handle().clone(),
            )),
        ));
        let catalog = Arc::new(
            self.application
                .query_project_presentation(
                    self.request.project_instance_id().clone(),
                    self.request.locale(),
                )
                .unwrap()
                .activity_panels
                .into_iter()
                .find(|panel| panel.panel_id == "nodes")
                .unwrap(),
        );
        cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::preferences::init(cx);
        });
        let (window, canvas) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    GraphCanvas::new(
                        services,
                        self.open(),
                        catalog,
                        self.request.locale().into(),
                        window,
                        cx,
                    )
                })
            })
            .unwrap()
        });
        (window, canvas)
    }
}

#[gpui_kit::test]
async fn node_drag_keeps_release_preview_and_commits_one_undoable_move(cx: &mut TestAppContext) {
    use crate::canvas::{GraphCommand, geometry};
    use gpui_kit::{MouseMoveEvent, MouseUpEvent};

    let fixture = Fixture::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    cx.update_window(window, |_, window, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.zoom = 2.;
            canvas.selected = canvas
                .graph
                .projection
                .nodes
                .iter()
                .map(|node| node.node_id)
                .collect();
            let press = point(px(40.), px(40.));
            canvas.begin_node(
                fixture.source,
                &MouseDownEvent {
                    button: MouseButton::Left,
                    position: press,
                    ..Default::default()
                },
                window,
                cx,
            );
            canvas.pointer_move(
                &MouseMoveEvent {
                    position: point(px(80.), px(60.)),
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                },
                cx,
            );
            let frame = canvas.preview.clone();
            assert_eq!(
                frame.position(fixture.source),
                Some(NodePosition { x: 20., y: 10. })
            );
            canvas.pointer_move(
                &MouseMoveEvent {
                    position: point(px(90.), px(80.)),
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(
                frame.position(fixture.source),
                Some(NodePosition { x: 20., y: 10. })
            );
            canvas.bounds.set(gpui_kit::Bounds::new(
                point(px(0.), px(0.)),
                gpui_kit::size(px(10.), px(10.)),
            ));
            canvas.pointer_up(
                &MouseUpEvent {
                    button: MouseButton::Left,
                    position: point(px(100.), px(100.)),
                    ..Default::default()
                },
                window,
                cx,
            );
            assert!(canvas.gesture.is_none());
            assert!(canvas.busy);
            canvas.cancel_gesture();
            for node in &canvas.graph.projection.nodes {
                assert_eq!(node.position, NodePosition { x: 0., y: 0. });
                assert_eq!(
                    geometry::position(node, &canvas.preview),
                    NodePosition { x: 30., y: 30. }
                );
            }
        });
    })
    .unwrap();
    cx.condition(&canvas, |canvas, _| !canvas.busy).await;
    canvas.update(cx, |canvas, cx| {
        assert!(
            canvas
                .graph
                .projection
                .nodes
                .iter()
                .all(|node| { node.position == NodePosition { x: 30., y: 30. } })
        );
        assert!(canvas.preview.position(fixture.source).is_none());
        assert!(canvas.graph.editing.can_undo);
        canvas.submit(GraphCommand::Undo, None, cx);
    });
    cx.condition(&canvas, |canvas, _| !canvas.busy).await;
    canvas.update(cx, |canvas, cx| {
        assert!(
            canvas
                .graph
                .projection
                .nodes
                .iter()
                .all(|node| { node.position == NodePosition { x: 0., y: 0. } })
        );
        assert!(
            !canvas.graph.editing.can_undo,
            "one release must create only one undo entry"
        );
        canvas.submit(GraphCommand::Redo, None, cx);
    });
    cx.condition(&canvas, |canvas, _| !canvas.busy).await;
    canvas.update(cx, |canvas, _| {
        assert!(
            canvas
                .graph
                .projection
                .nodes
                .iter()
                .all(|node| { node.position == NodePosition { x: 30., y: 30. } })
        );
    });
}

#[gpui_kit::test]
fn node_drag_return_to_origin_and_cancellation_do_not_edit(cx: &mut TestAppContext) {
    use gpui_kit::{MouseMoveEvent, MouseUpEvent};

    let fixture = Fixture::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    cx.update_window(window, |_, window, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.zoom = 1.;
            let version = canvas.graph.editing.version;
            canvas.selected = canvas
                .graph
                .projection
                .nodes
                .iter()
                .map(|node| node.node_id)
                .collect();
            let selected = canvas.selected.clone();
            let press = point(px(40.), px(40.));
            let down = MouseDownEvent {
                button: MouseButton::Left,
                position: press,
                ..Default::default()
            };
            let moving = MouseMoveEvent {
                position: point(px(100.), px(100.)),
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            };
            canvas.begin_node(fixture.source, &down, window, cx);
            canvas.pointer_move(&moving, cx);
            canvas.pointer_up(
                &MouseUpEvent {
                    button: MouseButton::Left,
                    position: press,
                    ..Default::default()
                },
                window,
                cx,
            );
            assert!(!canvas.busy);
            assert_eq!(
                canvas.selected, selected,
                "out-and-back must not reveal/collapse selection"
            );
            assert_eq!(canvas.graph.editing.version, version);
            assert!(!canvas.graph.editing.can_undo);
            assert!(canvas.preview.position(fixture.source).is_none());

            let managed = canvas
                .graph
                .projection
                .nodes
                .iter()
                .find(|node| node.node_id != fixture.source)
                .unwrap()
                .node_id;
            Arc::make_mut(&mut canvas.graph.projection)
                .nodes
                .iter_mut()
                .find(|node| node.node_id == managed)
                .unwrap()
                .capabilities
                .managed = true;
            canvas.begin_node(fixture.source, &down, window, cx);
            canvas.pointer_move(&moving, cx);
            assert_eq!(canvas.preview.mutations().len(), 1);
            assert!(canvas.preview.position(managed).is_none());
            canvas.cancel_gesture();
            assert!(canvas.gesture.is_none());
            assert!(canvas.preview.position(fixture.source).is_none());
            assert_eq!(canvas.graph.editing.version, version);

            canvas.begin_node(managed, &down, window, cx);
            canvas.pointer_move(&moving, cx);
            assert!(canvas.preview.mutations().is_empty());
            canvas.pointer_move(&MouseMoveEvent::default(), cx);
            assert!(canvas.gesture.is_none());
            assert!(canvas.preview.position(fixture.source).is_none());
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn node_drag_rejects_press_time_version_after_external_edit(cx: &mut TestAppContext) {
    use gpui_kit::MouseUpEvent;

    let fixture = Fixture::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    cx.update_window(window, |_, window, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.begin_node(
                fixture.source,
                &MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(40.), px(40.)),
                    ..Default::default()
                },
                window,
                cx,
            );
            fixture.edit(
                &canvas.graph,
                EditorGraphMutation::SetNodeLabel {
                    node_id: fixture.source,
                    label: Some("External edit".into()),
                },
            );
            canvas.pointer_up(
                &MouseUpEvent {
                    button: MouseButton::Left,
                    position: point(px(100.), px(100.)),
                    ..Default::default()
                },
                window,
                cx,
            );
            assert!(canvas.busy);
            assert!(canvas.preview.position(fixture.source).is_some());
        });
    })
    .unwrap();
    cx.condition(&canvas, |canvas, _| {
        !canvas.busy && canvas.refresh_task.is_none()
    })
    .await;
    canvas.update(cx, |canvas, _| {
        assert!(canvas.preview.position(fixture.source).is_none());
        assert!(canvas.gesture.is_none());
        let node = canvas
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.node_id == fixture.source)
            .unwrap();
        assert_eq!(node.position, NodePosition { x: 0., y: 0. });
        assert_eq!(node.display.user_label.as_deref(), Some("External edit"));
    });
}

#[test]
fn connection_install_tracks_move_reconnect_port_reorder_delete_and_restore() {
    use crate::canvas::{connections::ConnectionLayer, geometry::NodeMovement};
    use yss_graph_document::{ConnectionId, PortAddress};
    use yss_graph_editor::projection::EditorConnectionModel;
    use yss_node_protocol::PortDirection;

    let fixture = Fixture::new();
    let mut projection = fixture.open().projection.as_ref().clone();
    let mut source = projection
        .nodes
        .iter()
        .find(|node| node.node_id == fixture.source)
        .unwrap()
        .clone();
    let mut output = source
        .ports
        .iter()
        .find(|port| port.direction == PortDirection::Output)
        .unwrap()
        .clone();
    output.address = PortAddress::declared(source.node_id, "output".parse().unwrap());
    source.ports = vec![output.clone()].into_boxed_slice();
    let mut target = source.clone();
    target.node_id = NodeId::new();
    target.position = NodePosition { x: 600., y: 0. };
    let mut input = output.clone();
    input.direction = PortDirection::Input;
    input.address = PortAddress::declared(target.node_id, "input".parse().unwrap());
    target.ports = vec![input.clone()].into_boxed_slice();
    projection.nodes = vec![source, target].into_boxed_slice();
    let connection = ConnectionId::new();
    projection.connections = vec![EditorConnectionModel {
        connection_id: connection,
        output: output.address.clone(),
        input: input.address.clone(),
        order: None,
    }]
    .into_boxed_slice();
    let idle = NodeMovement::default();
    let midpoint = |layer: &ConnectionLayer, input: &PortAddress, movement: &NodeMovement| {
        (layer.port_position(&output.address, movement).unwrap()
            + layer.port_position(input, movement).unwrap())
            / 2.
    };
    let mut layer = ConnectionLayer::new(&projection);
    let original = midpoint(&layer, &input.address, &idle);
    assert_eq!(layer.hit_test(original, 1., &idle), Some(connection));
    layer.install(&projection);
    assert_eq!(layer.hit_test(original, 1., &idle), Some(connection));

    projection.nodes[1].position.y = 200.;
    layer.install(&projection);
    let moved = midpoint(&layer, &input.address, &idle);
    assert_eq!(layer.hit_test(moved, 1., &idle), Some(connection));
    assert_eq!(layer.hit_test(original, 1., &idle), None);

    for node in &mut projection.nodes {
        node.position.x += 50.;
        node.position.y += 200.;
    }
    layer.install(&projection);
    let translated = midpoint(&layer, &input.address, &idle);
    assert_eq!(layer.hit_test(translated, 1., &idle), Some(connection));
    assert_eq!(layer.hit_test(moved, 1., &idle), None);

    let mut movement = NodeMovement::new(
        projection
            .nodes
            .iter()
            .map(|node| (node.node_id, node.position))
            .collect(),
    );
    movement.set_displacement(point(px(80.), px(-50.)));
    let previewed = midpoint(&layer, &input.address, &movement);
    assert_eq!(previewed, translated + point(px(80.), px(-50.)));
    assert_eq!(layer.hit_test(previewed, 1., &movement), Some(connection));
    for node in &mut projection.nodes {
        node.position = movement.position(node.node_id).unwrap();
    }
    layer.install(&projection);
    assert_eq!(layer.hit_test(previewed, 1., &idle), Some(connection));

    let mut replacement = projection.nodes[1].clone();
    replacement.node_id = NodeId::new();
    replacement.position.y = 900.;
    let replacement_address = PortAddress::declared(replacement.node_id, "input".parse().unwrap());
    replacement.ports[0].address = replacement_address.clone();
    let mut nodes = projection.nodes.into_vec();
    nodes.push(replacement);
    projection.nodes = nodes.into_boxed_slice();
    projection.connections[0].input = replacement_address.clone();
    layer.install(&projection);
    let reconnected = midpoint(&layer, &replacement_address, &idle);
    assert_eq!(layer.hit_test(reconnected, 1., &idle), Some(connection));
    assert_eq!(layer.hit_test(previewed, 1., &idle), None);

    let before_reorder = layer.port_position(&replacement_address, &idle).unwrap();
    let mut earlier = projection.nodes[2].ports[0].clone();
    earlier.address =
        PortAddress::declared(projection.nodes[2].node_id, "earlier".parse().unwrap());
    projection.nodes[2].ports =
        vec![earlier, projection.nodes[2].ports[0].clone()].into_boxed_slice();
    layer.install(&projection);
    assert_eq!(
        layer.port_position(&replacement_address, &idle).unwrap(),
        before_reorder + point(px(0.), px(crate::canvas::geometry::PORT_HEIGHT))
    );
    let reordered = midpoint(&layer, &replacement_address, &idle);
    assert_eq!(layer.hit_test(reordered, 1., &idle), Some(connection));

    projection.nodes[0].ports[0].direction = PortDirection::Input;
    layer.install(&projection);
    assert_eq!(
        layer.port_direction(&output.address),
        Some(PortDirection::Input)
    );
    let reordered = midpoint(&layer, &replacement_address, &idle);
    assert_eq!(layer.hit_test(reordered, 1., &idle), Some(connection));

    let restored = projection.clone();
    projection.connections = Box::new([]);
    layer.install(&projection);
    assert_eq!(layer.hit_test(reordered, 1., &idle), None);
    layer.install(&restored);
    assert_eq!(layer.hit_test(reordered, 1., &idle), Some(connection));

    projection = restored.clone();
    projection.nodes = projection
        .nodes
        .into_vec()
        .into_iter()
        .filter(|node| node.node_id != replacement_address.node_id)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    layer.install(&projection);
    assert_eq!(layer.port_position(&replacement_address, &idle), None);
    assert_eq!(layer.hit_test(reordered, 1., &idle), None);
    layer.install(&restored);
    assert_eq!(layer.hit_test(reordered, 1., &idle), Some(connection));
}

#[test]
fn cached_node_geometry_tracks_inline_parameters_ports_and_compact_style() {
    use crate::canvas::{
        connections::ConnectionLayer,
        geometry::{NodeMovement, PARAMETER_CHROME, PARAMETER_HEIGHT, PORT_HEIGHT},
        presentation,
    };
    use yss_graph_document::PortAddress;
    use yss_node_protocol::ParameterPresentation;

    let fixture = Fixture::new();
    let mut projection = fixture.open().projection.as_ref().clone();
    let mut node = projection
        .nodes
        .iter()
        .find(|node| node.node_id == fixture.source)
        .unwrap()
        .clone();
    node.ports = vec![node.ports.first().unwrap().clone()].into_boxed_slice();
    for parameter in node
        .parameter_groups
        .iter_mut()
        .flat_map(|group| group.parameters.iter_mut())
    {
        parameter.presentation = ParameterPresentation::DetailPanel;
    }
    projection.nodes = vec![node].into_boxed_slice();
    projection.connections = Box::new([]);
    let mut layer = ConnectionLayer::new(&projection);
    let idle = NodeMovement::default();
    let original = projection.clone();
    let original_height = layer.node(fixture.source).unwrap().layout.height;
    let output = projection.nodes[0].ports[0].address.clone();
    let anchor = layer.port_position(&output, &idle).unwrap();

    projection.nodes[0].position = NodePosition { x: 100., y: 70. };
    assert!(presentation::same_content(&original, &projection));
    layer.install(&projection);
    assert_eq!(
        layer.node(fixture.source).unwrap().layout.height,
        original_height
    );
    assert_eq!(
        layer.port_position(&output, &idle),
        Some(anchor + point(px(100.), px(70.)))
    );

    projection.nodes[0]
        .parameter_groups
        .iter_mut()
        .flat_map(|group| group.parameters.iter_mut())
        .next()
        .unwrap()
        .presentation = ParameterPresentation::InlineAndDetail;
    assert!(!presentation::same_content(&original, &projection));
    layer.install(&projection);
    let parameter = PARAMETER_HEIGHT + PARAMETER_CHROME;
    assert_eq!(
        layer.node(fixture.source).unwrap().layout.height,
        original_height + parameter
    );
    assert_eq!(
        layer.port_position(&output, &idle).unwrap().y,
        anchor.y + px(70. + parameter)
    );

    let mut extra = projection.nodes[0].ports[0].clone();
    extra.address = PortAddress::declared(fixture.source, "additional".parse().unwrap());
    let extra_address = extra.address.clone();
    let mut ports = projection.nodes[0].ports.to_vec();
    ports.push(extra);
    projection.nodes[0].ports = ports.into_boxed_slice();
    layer.install(&projection);
    assert_eq!(
        layer.port_position(&extra_address, &idle).unwrap().y,
        layer.port_position(&output, &idle).unwrap().y + px(PORT_HEIGHT)
    );
    let bounds = layer
        .node(fixture.source)
        .unwrap()
        .layout
        .bounds(projection.nodes[0].position);
    let extra_anchor = layer.port_position(&extra_address, &idle).unwrap();
    assert_eq!(bounds.right(), extra_anchor.x);
    assert!(extra_anchor.y > bounds.top() && extra_anchor.y < bounds.bottom());
    projection.nodes[0].display.style_id = Some("builtin.reroute".into());
    layer.install(&projection);
    let layout = layer.node(fixture.source).unwrap().layout;
    assert_eq!((layout.width, layout.height), (20., 20.));
    // A reroute is a dot: every wire meets at its center, whatever its port rows.
    for address in [&output, &extra_address] {
        assert_eq!(
            layer.port_position(address, &idle),
            Some(point(px(110.), px(80.)))
        );
    }
}

#[gpui_kit::test]
async fn inline_draft_keeps_focus_across_viewport_and_commits_on_return(cx: &mut TestAppContext) {
    use gpui_kit::test::TestWindowExt;
    let fixture = Fixture::new();
    let graph = fixture.open();
    fixture.edit(
        &graph,
        EditorGraphMutation::DeleteNodes {
            node_ids: graph
                .projection
                .nodes
                .iter()
                .map(|node| node.node_id)
                .collect(),
        },
    );
    fixture.edit(
        &fixture.open(),
        EditorGraphMutation::CreateNode {
            descriptor: yss_node_catalog::NodeCreation::Static {
                node_type_id: "yssbi.numeric.subtract".parse().unwrap(),
            },
            position: NodePosition { x: 40., y: 40. },
            connect_from: None,
            parameters: Default::default(),
            port_counts: Default::default(),
            user_label: None,
        },
    );
    let graph = fixture.open();
    for address in graph
        .projection
        .nodes
        .iter()
        .flat_map(|node| node.ports.iter())
        .filter(|port| port.direction == yss_node_protocol::PortDirection::Input)
        .map(|port| port.address.clone())
    {
        fixture.edit(
            &fixture.open(),
            EditorGraphMutation::SetLiteral {
                address,
                literal: Some(serde_json::json!(0)),
            },
        );
    }
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    let address = canvas.read_with(cx, |canvas, _| {
        canvas
            .graph
            .projection
            .nodes
            .iter()
            .flat_map(|node| node.ports.iter())
            .find(|port| {
                crate::canvas::scalar_input_type(port)
                    == Some(yss_data_contract::SemanticType::Numeric)
            })
            .unwrap()
            .address
            .clone()
    });
    let id = gpui_kit::SharedString::from(format!("port-value-{address}"));
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click(id.clone(), cx);
        window.press("ctrl-a", cx);
        window.input("42", cx);
        assert_eq!(window.find(id.clone()).value(), Some("42"));
        canvas.update(cx, |canvas, cx| {
            canvas.offset = point(px(-10_000.), px(-10_000.));
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find(id.clone()).focused(), Some(true));
        assert_eq!(window.find(id.clone()).value(), Some("42"));
        canvas.update(cx, |canvas, cx| {
            canvas.offset = point(px(0.), px(0.));
            cx.notify();
        });
        window.render_frame(cx);
        assert_eq!(window.find(id.clone()).focused(), Some(true));
        window.press("enter", cx);
    })
    .unwrap();
    cx.condition(&canvas, |canvas, _| {
        !canvas.busy && canvas.refresh_task.is_none()
    })
    .await;
    canvas.read_with(cx, |canvas, _| {
        let port = canvas
            .port_details
            .as_ref()
            .unwrap()
            .port(&address)
            .unwrap();
        assert_eq!(
            port.input.as_ref().unwrap().literal_override,
            Some(serde_json::json!(42))
        );
        assert!(!canvas.has_dirty_port_inputs());
    });
}

#[gpui_kit::test]
fn empty_canvas_button_and_keyboard_open_palette_and_restore_focus(cx: &mut TestAppContext) {
    use gpui_kit::{KeyBinding, test::TestWindowExt};
    let fixture = Fixture::new();
    let graph = fixture.open();
    fixture.edit(
        &graph,
        EditorGraphMutation::DeleteNodes {
            node_ids: graph
                .projection
                .nodes
                .iter()
                .map(|node| node.node_id)
                .collect(),
        },
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new(
            "ctrl-space",
            crate::canvas::OpenNodePalette,
            Some("GraphCanvas && !Input"),
        )])
    });
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        window.click("add-first-node", cx);
        assert!(canvas.read(cx).palette.is_some());
        window.press("escape", cx);
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        assert!(canvas.read(cx).palette.is_none());
        let focus = canvas.read(cx).focus.clone();
        window.focus(&focus, cx);
        window.render_frame(cx);
        window.press("ctrl-space", cx);
        assert!(canvas.read(cx).palette.is_some());
        assert!(!canvas.read(cx).focus.is_focused(window));
        window.press("escape", cx);
    })
    .unwrap();
    cx.update_window(window, |_, window, cx| {
        assert!(canvas.read(cx).palette.is_none());
        assert!(canvas.read(cx).focus.is_focused(window));
    })
    .unwrap();
}

#[gpui_kit::test]
fn pointer_targets_follow_painted_geometry_and_raise_the_selection(cx: &mut TestAppContext) {
    use crate::canvas::CanvasTarget;
    use gpui_kit::{Bounds, size};
    use yss_node_protocol::PortDirection;

    let fixture = Fixture::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    cx.update_window(window, |_, _, cx| {
        canvas.update(cx, |canvas, _| {
            canvas.bounds.set(Bounds::new(
                point(px(0.), px(0.)),
                size(px(1200.), px(800.)),
            ));
            canvas.offset = point(px(40.), px(40.));
            canvas.zoom = canvas.font_scale;
            let origin = point(px(0.), px(0.));
            // Both fixture nodes sit at the origin; the later one is drawn and picked on top.
            let nodes = canvas
                .graph
                .projection
                .nodes
                .iter()
                .map(|node| node.node_id)
                .collect::<Vec<_>>();
            let (bottom, top) = (nodes[0], nodes[1]);
            let rect = {
                let layer = &canvas.connection_layer;
                let node = &canvas.graph.projection.nodes[1];
                canvas.node_screen_rect(node, layer.node(top).unwrap(), origin)
            };
            let header = point(rect.left() + px(100.), rect.top() + px(10.));
            assert_eq!(canvas.target_at(header), Some(CanvasTarget::Node(top)));
            assert_eq!(
                canvas.target_at(canvas.status_center(rect)),
                Some(CanvasTarget::Status(top))
            );
            canvas.selected = [bottom].into();
            assert_eq!(canvas.target_at(header), Some(CanvasTarget::Node(bottom)));

            let (owner, port) = canvas
                .graph
                .projection
                .nodes
                .iter()
                .find_map(|node| {
                    let port = node
                        .ports
                        .iter()
                        .find(|port| port.direction == PortDirection::Output)?;
                    Some((node.node_id, port.address.clone()))
                })
                .unwrap();
            canvas.selected = [owner].into();
            let anchor = canvas
                .connection_layer
                .port_position(&port, &canvas.preview)
                .unwrap();
            // Pins reach a little past the card edge they straddle.
            let outside = canvas.offset + anchor * canvas.zoom + point(px(6.), px(0.));
            assert_eq!(canvas.target_at(outside), Some(CanvasTarget::Port(port)));
            assert_eq!(canvas.target_at(point(px(1100.), px(700.))), None);
            assert_eq!(canvas.target_at(point(px(1300.), px(10.))), None);
        });
    })
    .unwrap();
}

#[gpui_kit::test]
async fn reroute_dot_is_the_node_body_and_its_sides_pull_wires(cx: &mut TestAppContext) {
    use crate::canvas::{CanvasTarget, GraphEditCommand};
    use gpui_kit::{Bounds, MouseMoveEvent, MouseUpEvent, size};
    use yss_node_protocol::PortDirection;

    let fixture = Fixture::new();
    let graph = fixture.open();
    let output = graph
        .projection
        .nodes
        .iter()
        .find(|node| node.node_id == fixture.source)
        .and_then(|node| {
            node.ports
                .iter()
                .find(|port| port.direction == PortDirection::Output)
        })
        .unwrap()
        .address
        .clone();
    fixture.edit(
        &graph,
        EditorGraphMutation::CreateNode {
            descriptor: yss_node_catalog::NodeCreation::Static {
                node_type_id: "yssbi.debug.view".parse().unwrap(),
            },
            position: NodePosition { x: 600., y: 0. },
            connect_from: Some(output),
            parameters: Default::default(),
            port_counts: Default::default(),
            user_label: None,
        },
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (window, canvas) = fixture.canvas(&runtime, cx);
    let click = point(px(420.), px(160.));
    cx.update_window(window, |_, window, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.bounds.set(Bounds::new(
                point(px(0.), px(0.)),
                size(px(1200.), px(800.)),
            ));
            canvas.offset = point(px(0.), px(0.));
            canvas.zoom = canvas.font_scale;
            let connection = canvas.graph.projection.connections[0].connection_id;
            canvas.begin_connection(
                connection,
                &MouseDownEvent {
                    button: MouseButton::Left,
                    position: click,
                    click_count: 2,
                    ..Default::default()
                },
                window,
                cx,
            );
        });
    })
    .unwrap();
    cx.condition(&canvas, |canvas, _| !canvas.busy).await;

    let reroute = canvas.update(cx, |canvas, _| {
        let reroute = canvas
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.display.style_id.as_deref() == Some("builtin.reroute"))
            .unwrap();
        let rect = canvas.node_screen_rect(
            reroute,
            canvas.connection_layer.node(reroute.node_id).unwrap(),
            point(px(0.), px(0.)),
        );
        // The dot lands where the wire was double-clicked, and both wires meet at it.
        assert_eq!(rect.center(), click);
        let port = |direction| {
            reroute
                .ports
                .iter()
                .find(|port| port.direction == direction)
                .unwrap()
                .address
                .clone()
        };
        for direction in [PortDirection::Input, PortDirection::Output] {
            let anchor = canvas
                .connection_layer
                .port_position(&port(direction), &canvas.preview)
                .unwrap();
            assert_eq!(canvas.offset + anchor * canvas.zoom, click);
        }
        // The dot is the node itself; its sides still start wires in either direction.
        assert_eq!(
            canvas.target_at(click),
            Some(CanvasTarget::Node(reroute.node_id))
        );
        assert_eq!(
            canvas.target_at(point(rect.right() + px(4.), click.y)),
            Some(CanvasTarget::Port(port(PortDirection::Output)))
        );
        assert_eq!(
            canvas.target_at(point(rect.left() - px(4.), click.y)),
            Some(CanvasTarget::Port(port(PortDirection::Input)))
        );
        reroute.node_id
    });

    let moved = click + point(px(30.), px(20.));
    cx.update_window(window, |_, window, cx| {
        canvas.update(cx, |canvas, cx| {
            canvas.begin_pane(
                &MouseDownEvent {
                    button: MouseButton::Left,
                    position: click,
                    click_count: 1,
                    ..Default::default()
                },
                window,
                cx,
            );
            assert_eq!(canvas.selected, [reroute].into());
            assert!(canvas.is_edit_command_enabled(GraphEditCommand::Cut));
            assert!(canvas.is_edit_command_enabled(GraphEditCommand::Delete));
            canvas.pointer_move(
                &MouseMoveEvent {
                    position: moved,
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                },
                cx,
            );
            canvas.pointer_up(
                &MouseUpEvent {
                    button: MouseButton::Left,
                    position: moved,
                    ..Default::default()
                },
                window,
                cx,
            );
        });
    })
    .unwrap();
    cx.condition(&canvas, |canvas, _| !canvas.busy).await;
    canvas.update(cx, |canvas, _| {
        let node = canvas
            .graph
            .projection
            .nodes
            .iter()
            .find(|node| node.node_id == reroute)
            .unwrap();
        let rect = canvas.node_screen_rect(
            node,
            canvas.connection_layer.node(reroute).unwrap(),
            point(px(0.), px(0.)),
        );
        assert_eq!(rect.center(), moved);
    });
}
