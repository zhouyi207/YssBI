//! Frame cost of a large graph with a real text system.
//!
//! `cargo test -p yss-graph-ui --lib canvas::benchmark -- --ignored --nocapture`
use super::GraphCanvas;
use crate::{GraphServices, OpenedGraph};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, Entity, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, TestDispatcher,
    Window, WindowBounds, WindowOptions, point, px, size,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use yss_application::{graph::open::OpenGraphRequest, session::ApplicationState};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphResourceKind, GraphResourcePath, NodeId,
    NodePosition, PortAddress,
};
use yss_node_protocol::PortKey;

const COLUMNS: usize = 16;
const ROWS: usize = 20;

struct Project {
    directory: tempfile::TempDir,
    application: ApplicationState,
    request: OpenGraphRequest,
}

fn project() -> Project {
    let directory = tempfile::tempdir().unwrap();
    let path = GraphResourcePath::new("events/Large.yssbi-event").unwrap();
    let mut data = yss_project_model::ProjectData::new();
    let mut resource =
        yss_project_model::GraphResourceDocument::new("Large", GraphResourceKind::EventGraph);
    let document = Arc::make_mut(&mut resource.document);
    let ids = (0..COLUMNS)
        .map(|_| (0..ROWS).map(|_| NodeId::new()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let port = |node, key: &str| PortAddress::declared(node, PortKey::new(key).unwrap());
    for (column, nodes) in ids.iter().enumerate() {
        for (row, id) in nodes.iter().enumerate() {
            let node_type = if column == 0 {
                "yssbi.constant.pi"
            } else {
                "yssbi.numeric.subtract"
            };
            document.nodes.insert(
                *id,
                DocumentNode {
                    id: *id,
                    node_type: node_type.parse().unwrap(),
                    position: NodePosition {
                        x: column as f64 * 360.,
                        y: row as f64 * 160.,
                    },
                    parameters: Default::default(),
                    user_label: None,
                },
            );
            if column == 0 {
                continue;
            }
            let previous = &ids[column - 1];
            let output = |node| port(node, if column == 1 { "value" } else { "result" });
            let mut connect = |output, input| {
                let id = ConnectionId::new();
                document.connections.insert(
                    id,
                    DocumentConnection {
                        id,
                        output,
                        input,
                        order: None,
                    },
                );
            };
            connect(output(previous[row]), port(*id, "left"));
            // Odd rows keep an unconnected scalar input, which shows an inline field.
            if row % 2 == 0 {
                connect(output(previous[(row + 1) % ROWS]), port(*id, "right"));
            }
        }
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
    Project {
        directory,
        application,
        request,
    }
}

fn open_canvas(
    project: &Project,
    runtime: &tokio::runtime::Runtime,
    cx: &mut TestAppContext,
) -> (AnyWindowHandle, Entity<GraphCanvas>) {
    cx.executor().allow_parking();
    let tasks = Arc::new(yss_desktop_services::ApplicationTasks::new(
        project.application.clone(),
        runtime.handle().clone(),
        Default::default(),
    ));
    let services = Arc::new(GraphServices::new(
        tasks,
        Arc::new(yss_ui::layout_store::LayoutStore::new(
            project.directory.path(),
            runtime.handle().clone(),
        )),
    ));
    let catalog = Arc::new(
        project
            .application
            .query_project_presentation(
                project.request.project_instance_id().clone(),
                project.request.locale(),
            )
            .unwrap()
            .activity_panels
            .into_iter()
            .find(|panel| panel.panel_id == "nodes")
            .unwrap(),
    );
    let graph = OpenedGraph::from_open(
        project
            .application
            .open_graph(project.request.clone())
            .unwrap(),
        project.request.locale(),
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        yss_ui::preferences::init(cx);
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(1600.), px(1000.)),
        ))),
        ..Default::default()
    };
    let locale = project.request.locale().to_owned();
    cx.update(|cx| {
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| GraphCanvas::new(services, graph, catalog, locale, window, cx))
        })
        .unwrap()
    })
}

#[test]
#[ignore = "measures frame cost; run explicitly with --ignored --nocapture"]
fn large_graph_frame_cost() {
    gpui_kit::run_test_once(
        0,
        Box::new(|dispatcher: TestDispatcher| {
            let text_system = gpui_kit::platform::current_platform(true).text_system();
            let mut cx = TestAppContext::build_with_text_system(dispatcher, None, text_system);
            let project = project();
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let (window, canvas) = open_canvas(&project, &runtime, &mut cx);
            cx.run_until_parked();
            for (label, zoom) in [("fit", None), ("100%", Some(1.))] {
                cx.update_window(window, |_, window, cx| {
                    canvas.update(cx, |canvas, cx| {
                        canvas.reveal_graph(window, cx);
                        if let Some(zoom) = zoom {
                            canvas.zoom = zoom * canvas.font_scale;
                        }
                        cx.notify();
                    })
                })
                .unwrap();
                scenarios(&mut cx, window, &canvas, label);
            }
            cx.update(|cx| cx.quit());
            cx.run_until_parked();
        }),
    );
}

fn scenarios(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    canvas: &Entity<GraphCanvas>,
    label: &str,
) {
    let center = point(px(800.), px(500.));
    let visible = canvas.read_with(cx, |canvas, _| visible_nodes(canvas));
    println!("--- {label}: {visible} visible nodes");
    measure(
        cx,
        window,
        canvas,
        &format!("redraw {label}"),
        40,
        |_, _, _, cx| {
            cx.notify();
        },
    );
    measure(
        cx,
        window,
        canvas,
        &format!("pan {label}"),
        40,
        |index, canvas, window, cx| {
            if index == 0 {
                canvas.begin_pan(None, &press(center, MouseButton::Middle), window, cx);
            }
            canvas.pointer_move(&moved(center, index, MouseButton::Middle), cx);
        },
    );
    canvas.update(cx, |canvas, _| canvas.cancel_gesture());
    let node = canvas.read_with(cx, |canvas, _| {
        canvas
            .graph
            .projection
            .nodes
            .get(COLUMNS * ROWS / 2)
            .map(|node| node.node_id)
    });
    if let Some(node) = node {
        measure(
            cx,
            window,
            canvas,
            &format!("drag node {label}"),
            40,
            |index, canvas, window, cx| {
                if index == 0 {
                    canvas.begin_node(node, &press(center, MouseButton::Left), window, cx);
                }
                canvas.pointer_move(&moved(center, index, MouseButton::Left), cx);
            },
        );
        canvas.update(cx, |canvas, _| canvas.cancel_gesture());
    }
    measure(
        cx,
        window,
        canvas,
        &format!("zoom {label}"),
        40,
        |index, canvas, window, cx| {
            // Wheel zoom in, then back out: every frame lands on a new scale.
            let delta = if index < 20 { 20. } else { -20. };
            canvas.zoom_at_pointer(
                &ScrollWheelEvent {
                    position: center,
                    delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
                    ..Default::default()
                },
                window,
                cx,
            );
        },
    );
    let hover = Instant::now();
    let probes = 400;
    canvas.update(cx, |canvas, cx| {
        for index in 0..probes {
            let position = point(
                px(40. + (index % 40) as f32 * 38.),
                px(40. + (index / 40) as f32 * 92.),
            );
            canvas.update_hover(position, cx);
        }
    });
    println!(
        "{:<20} {:>9.2?} per pointer move (hit test only)",
        format!("hover {label}"),
        hover.elapsed() / probes
    );
}

fn visible_nodes(canvas: &GraphCanvas) -> usize {
    let size = canvas.bounds.get().size;
    let layer = &canvas.connection_layer;
    canvas
        .graph
        .projection
        .nodes
        .iter()
        .zip(layer.nodes())
        .filter(|(node, geometry)| {
            let rect = geometry.layout.bounds(node.position);
            let origin = canvas.offset + rect.origin * canvas.zoom;
            origin.x + rect.size.width * canvas.zoom >= px(0.)
                && origin.y + rect.size.height * canvas.zoom >= px(0.)
                && origin.x <= size.width
                && origin.y <= size.height
        })
        .count()
}

fn press(position: Point<Pixels>, button: MouseButton) -> MouseDownEvent {
    MouseDownEvent {
        button,
        position,
        click_count: 1,
        ..Default::default()
    }
}

fn moved(center: Point<Pixels>, index: usize, button: MouseButton) -> MouseMoveEvent {
    MouseMoveEvent {
        position: center + point(px(3. * (index + 1) as f32), px(2. * (index + 1) as f32)),
        pressed_button: Some(button),
        ..Default::default()
    }
}

fn measure(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    canvas: &Entity<GraphCanvas>,
    label: &str,
    frames: usize,
    mut step: impl FnMut(usize, &mut GraphCanvas, &mut Window, &mut Context<GraphCanvas>),
) {
    cx.update_window(window, |_, window, _| window.reset_layout_stats())
        .unwrap();
    let mut times = Vec::with_capacity(frames);
    for index in 0..frames {
        let start = Instant::now();
        cx.update_window(window, |_, window, cx| {
            canvas.update(cx, |canvas, cx| step(index, canvas, window, cx))
        })
        .unwrap();
        times.push(start.elapsed());
    }
    let stats = cx
        .update_window(window, |_, window, _| window.layout_stats())
        .unwrap();
    let quads = cx
        .update_window(window, |_, window, _| window.painted_quads().len())
        .unwrap();
    times.sort();
    let count = frames as u32;
    let mean = times.iter().sum::<Duration>() / count;
    println!(
        "{label:<20} mean {mean:>9.2?} p50 {:>9.2?} max {:>9.2?} | build {:>9.2?} prepaint {:>9.2?} paint {:>9.2?} | layout nodes {} created {} | shaped {} | quads {quads}",
        times[frames / 2],
        times[frames - 1],
        stats.build_time / count,
        stats.prepaint_time / count,
        stats.paint_time / count,
        (stats.nodes_created + stats.nodes_reused) / frames as u64,
        stats.nodes_created / frames as u64,
        stats.lines_shaped / frames as u64,
    );
}
