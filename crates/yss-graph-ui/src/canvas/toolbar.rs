//! Compact graph controls live outside the pointer-interaction surface.
use super::{GraphCanvas, GraphEditCommand, ResultEntry, commands::*, result_search::Picker};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    App, Context, Entity, IntoElement, Render, Subscription, WeakEntity, Window, div, prelude::*,
    px,
};
use std::rc::Rc;
use yss_application::graph::run::RunIdentity;
use yss_graph_document::GraphResourcePath;
use yss_graph_execution::identity::ExecutionSessionId;
use yss_project_identity::ProjectInstanceId;

pub(super) struct Toolbar {
    owner: WeakEntity<GraphCanvas>,
    picker: Entity<Picker>,
    display: Option<Display>,
    _subscriptions: Vec<Subscription>,
}

// Only rendered values and retained callback/search context belong here. In particular,
// projection/edit revisions, preview geometry and pointer state are not dependencies.
struct Display {
    project: ProjectInstanceId,
    path: GraphResourcePath,
    edit_session: uuid::Uuid,
    execution_session: ExecutionSessionId,
    save: bool,
    undo: bool,
    redo: bool,
    editable: bool,
    run_node: bool,
    running: bool,
    run_unavailable: Option<&'static str>,
    sync_status: Option<&'static str>,
    clear_run: Option<RunIdentity>,
    inspect: bool,
    results: Rc<[ResultEntry]>,
    locale: &'static str,
}

impl Display {
    fn new(canvas: &GraphCanvas, cx: &App) -> Self {
        Self {
            project: canvas.graph.project.clone(),
            path: canvas.graph.projection.graph_path.clone(),
            edit_session: canvas.graph.editing.version.session_id,
            execution_session: canvas.graph.results.execution_session_id,
            save: canvas.can_edit() && canvas.dirty(cx),
            undo: canvas.is_edit_command_enabled(GraphEditCommand::Undo),
            redo: canvas.is_edit_command_enabled(GraphEditCommand::Redo),
            editable: canvas.can_edit(),
            run_node: canvas.can_run() && canvas.selected.len() == 1,
            running: canvas.is_running(),
            run_unavailable: canvas.graph_run_unavailable_reason(cx),
            sync_status: canvas.execution_sync_status(),
            clear_run: canvas.clearable_run().cloned(),
            inspect: !canvas.graph.results.outputs.is_empty(),
            results: canvas.result_entries().clone(),
            locale: yss_ui::text::locale(),
        }
    }

    fn same_context(&self, canvas: &GraphCanvas) -> bool {
        self.project == canvas.graph.project
            && self.path == canvas.graph.projection.graph_path
            && self.edit_session == canvas.graph.editing.version.session_id
            && self.execution_session == canvas.graph.results.execution_session_id
    }

    fn matches(&self, canvas: &GraphCanvas, cx: &App) -> bool {
        self.same_context(canvas)
            && self.save == (canvas.can_edit() && canvas.dirty(cx))
            && self.undo == canvas.is_edit_command_enabled(GraphEditCommand::Undo)
            && self.redo == canvas.is_edit_command_enabled(GraphEditCommand::Redo)
            && self.editable == canvas.can_edit()
            && self.run_node == (canvas.can_run() && canvas.selected.len() == 1)
            && self.running == canvas.is_running()
            && self.run_unavailable == canvas.graph_run_unavailable_reason(cx)
            && self.sync_status == canvas.execution_sync_status()
            && self.clear_run.as_ref() == canvas.clearable_run()
            && self.inspect == !canvas.graph.results.outputs.is_empty()
            && Rc::ptr_eq(&self.results, canvas.result_entries())
            && self.locale == yss_ui::text::locale()
    }
}

impl Toolbar {
    pub(super) fn new(
        owner: &Entity<GraphCanvas>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| Picker::new(owner, window, cx));
        let subscriptions = vec![
            cx.observe_in(owner, window, |toolbar, owner, window, cx| {
                let canvas = owner.read(cx);
                if toolbar
                    .display
                    .as_ref()
                    .is_some_and(|display| display.matches(canvas, cx))
                {
                    return;
                }
                let context_changed = toolbar
                    .display
                    .as_ref()
                    .is_some_and(|display| !display.same_context(canvas));
                toolbar.display = Some(Display::new(canvas, cx));
                if context_changed {
                    toolbar
                        .picker
                        .update(cx, |picker, cx| picker.reset(window, cx));
                }
                cx.notify();
            }),
            cx.observe(&picker, |_, _, cx| cx.notify()),
            cx.observe_global::<gpui_kit::component::Theme>(|_, cx| cx.notify()),
        ];
        Self {
            owner: owner.downgrade(),
            picker,
            display: None,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn initialize(&mut self, canvas: &GraphCanvas, cx: &App) {
        self.display = Some(Display::new(canvas, cx));
    }
}

impl Render for Toolbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Render and listeners still belong to GraphCanvas; this view only owns
        // the invalidation boundary and the search picker's retained lifetime.
        self.owner
            .update(cx, |canvas, cx| {
                canvas
                    .render_toolbar(&self.picker, window, cx)
                    .into_any_element()
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}

impl GraphCanvas {
    pub(super) fn render_toolbar(
        &self,
        picker: &Entity<Picker>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let run_node = self.can_run() && self.selected.len() == 1;
        let running = self.is_running();
        let run_unavailable = self.graph_run_unavailable_reason(cx);
        let clear_run = self.clearable_run().cloned();
        let event_graph = self.graph.projection.graph_path.kind()
            == yss_graph_document::GraphResourceKind::EventGraph;
        let focus = self.focus.clone();
        let owner = cx.weak_entity();
        let inspect = !self.graph.results.outputs.is_empty();
        div()
            .w_full()
            .h(yss_ui::appearance::HEADER_HEIGHT)
            .flex_shrink_0()
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .bg(cx.theme().table)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("save-graph")
                    .small()
                    .ghost()
                    .icon(IconName::Save)
                    .tooltip(yss_ui::text::t("native.canvas.saveShortcut"))
                    .accessibility_label(yss_ui::text::t("native.canvas.saveShortcut"))
                    .disabled(!self.can_edit() || !self.dirty(cx))
                    .on_click(
                        cx.listener(|view, _, _, cx| view.submit(GraphCommand::Save, None, cx)),
                    ),
            )
            .child(div().h_4().w(px(1.)).mx_1().bg(cx.theme().border))
            .child(
                Button::new("undo-graph")
                    .small()
                    .ghost()
                    .icon(IconName::Undo2)
                    .tooltip(yss_ui::text::t("native.canvas.undoShortcut"))
                    .accessibility_label(yss_ui::text::t("native.canvas.undoShortcut"))
                    .disabled(!self.is_edit_command_enabled(GraphEditCommand::Undo))
                    .on_click(cx.listener(|view, _, _, cx| view.edit(GraphEditCommand::Undo, cx))),
            )
            .child(
                Button::new("redo-graph")
                    .small()
                    .ghost()
                    .icon(IconName::Redo2)
                    .tooltip(yss_ui::text::t("native.canvas.redoShortcut"))
                    .accessibility_label(yss_ui::text::t("native.canvas.redoShortcut"))
                    .disabled(!self.is_edit_command_enabled(GraphEditCommand::Redo))
                    .on_click(cx.listener(|view, _, _, cx| view.edit(GraphEditCommand::Redo, cx))),
            )
            .when(event_graph, |bar| {
                bar.child(self.result_search(picker, window, cx))
            })
            .child(div().flex_1().min_w_0())
            .when_some(self.execution_sync_status(), |view, status| {
                view.child(
                    div()
                        .max_w(gpui_kit::rems(12.5))
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::t(status)),
                )
            })
            .child(
                Button::new("graph-options")
                    .small()
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .tooltip(yss_ui::text::t("native.canvas.moreActions"))
                    .accessibility_label(yss_ui::text::t("native.canvas.moreActions"))
                    .dropdown_menu(move |menu, _, _| {
                        menu.action_context(focus.clone())
                            .item(
                                PopupMenuItem::new(yss_ui::text::t(
                                    "panel.assistantToolNames.inspect_result",
                                ))
                                .disabled(!inspect)
                                .on_click(
                                    yss_ui::inspector::owner_command(
                                        owner.clone(),
                                        |view, _, cx| {
                                            view.inspect_results(cx);
                                        },
                                    ),
                                ),
                            )
                            .item(
                                PopupMenuItem::new(yss_ui::text::t(
                                    "canvas.clearExecutionArtifacts",
                                ))
                                .disabled(clear_run.is_none())
                                .on_click(
                                    yss_ui::inspector::owner_command(owner.clone(), {
                                        let clear_run = clear_run.clone();
                                        move |view, _, cx| {
                                            if let Some(run) = &clear_run {
                                                view.clear_run_notice(run, cx);
                                            }
                                        }
                                    }),
                                ),
                            )
                            .separator()
                            .when(event_graph, |menu| {
                                menu.menu_with_enable(
                                    yss_ui::text::t("contextMenu.node.runNode"),
                                    Box::new(RunCurrentNode),
                                    run_node,
                                )
                                .menu_with_enable(
                                    yss_ui::text::t("contextMenu.node.runTo"),
                                    Box::new(RunToNode),
                                    run_node,
                                )
                                .separator()
                            })
                            .menu(
                                yss_ui::text::t("native.canvas.refreshRun"),
                                Box::new(RefreshRunState),
                            )
                            .menu(
                                yss_ui::text::t("native.canvas.resetView"),
                                Box::new(FrameGraph),
                            )
                    }),
            )
            .child(
                Button::new("run-graph")
                    .small()
                    .when(!running, |button| button.primary())
                    .when(running, |button| button.ghost())
                    .icon(if running {
                        IconName::Square
                    } else {
                        IconName::Play
                    })
                    .label(if running {
                        yss_ui::text::t("common.cancel")
                    } else {
                        yss_ui::text::t("bayes.actions.run")
                    })
                    .tooltip(if running {
                        yss_ui::text::t("native.canvas.cancelShortcut")
                    } else if let Some(reason) = run_unavailable {
                        yss_ui::text::t(reason)
                    } else {
                        yss_ui::text::t("native.canvas.runShortcut")
                    })
                    .disabled(!running && run_unavailable.is_some())
                    .on_click(cx.listener(|view, _, _, cx| {
                        if view.is_running() {
                            view.cancel_run(cx);
                        } else {
                            view.run_graph(yss_application::graph::run::RunDemand::Default, cx);
                        }
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions, point};
    use std::sync::Arc;
    use yss_application::{graph::open::OpenGraphRequest, session::ApplicationState};
    use yss_graph_document::{DocumentNode, GraphResourceKind, NodeId, NodePosition};

    #[gpui_kit::test]
    fn display_tracks_commands_and_sessions_without_tracking_movement(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = GraphResourcePath::new("events/Toolbar.yssbi-event").unwrap();
        let mut data = yss_project_model::ProjectData::new();
        let mut resource =
            yss_project_model::GraphResourceDocument::new("Toolbar", GraphResourceKind::EventGraph);
        let node = NodeId::new();
        Arc::make_mut(&mut resource.document).nodes.insert(
            node,
            DocumentNode {
                id: node,
                node_type: "yssbi.dataframe.source.get".parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: [(
                    "dataframe".parse().unwrap(),
                    serde_json::json!("missing-data"),
                )]
                .into(),
                user_label: None,
            },
        );
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
        let locale = yss_ui::text::locale();
        let graph = crate::OpenedGraph::from_open(
            application
                .open_graph(OpenGraphRequest::new(project.clone(), path, 0, locale))
                .unwrap(),
            locale,
        );
        let catalog = Arc::new(
            application
                .query_project_presentation(project, locale)
                .unwrap()
                .activity_panels
                .into_iter()
                .find(|panel| panel.panel_id == "nodes")
                .unwrap(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let tasks = Arc::new(yss_desktop_services::ApplicationTasks::new(
            application,
            runtime.handle().clone(),
            Default::default(),
        ));
        let services = Arc::new(crate::GraphServices::new(
            tasks,
            Arc::new(yss_ui::layout_store::LayoutStore::new(
                directory.path(),
                runtime.handle().clone(),
            )),
        ));
        cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::preferences::init(cx);
        });
        cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let mut canvas =
                        GraphCanvas::new(services, graph, catalog, locale.into(), window, cx);
                    canvas.selected.insert(node);
                    for command in [
                        GraphEditCommand::Copy,
                        GraphEditCommand::Cut,
                        GraphEditCommand::Paste,
                        GraphEditCommand::Delete,
                    ] {
                        assert!(canvas.is_edit_command_enabled(command));
                    }
                    let idle = Display::new(&canvas, cx);
                    canvas.preview = super::super::geometry::NodeMovement::new(
                        [(node, NodePosition { x: 0., y: 0. })].into(),
                    );
                    canvas.preview.set_displacement(point(px(30.), px(40.)));
                    canvas.offset = point(px(20.), px(10.));
                    canvas.zoom *= 1.25;
                    assert!(idle.matches(&canvas, cx));

                    // Persisted position-only replacement is not itself a display dependency.
                    Arc::make_mut(&mut canvas.graph.projection).nodes[0].position =
                        NodePosition { x: 30., y: 40. };
                    canvas.graph.editing.version.revision =
                        yss_project_identity::ResourceRevision::new(100);
                    assert!(idle.matches(&canvas, cx));

                    canvas.graph.editing.dirty = true;
                    assert!(!idle.matches(&canvas, cx));
                    let dirty = Display::new(&canvas, cx);
                    canvas.graph.editing.can_undo = true;
                    assert!(!dirty.matches(&canvas, cx));
                    let undo = Display::new(&canvas, cx);
                    canvas.graph.editing.can_redo = true;
                    assert!(!undo.matches(&canvas, cx));
                    let history = Display::new(&canvas, cx);
                    canvas.selected.clear();
                    assert!(!history.matches(&canvas, cx));
                    let unselected = Display::new(&canvas, cx);
                    canvas.busy = true;
                    for command in [
                        GraphEditCommand::Undo,
                        GraphEditCommand::Redo,
                        GraphEditCommand::Copy,
                        GraphEditCommand::Cut,
                        GraphEditCommand::Paste,
                        GraphEditCommand::Delete,
                    ] {
                        assert!(!canvas.is_edit_command_enabled(command));
                    }
                    assert!(!unselected.matches(&canvas, cx));
                    canvas.busy = false;
                    assert!(unselected.matches(&canvas, cx));

                    canvas.graph.editing.version.session_id = uuid::Uuid::new_v4();
                    assert!(!unselected.same_context(&canvas));
                    let reopened = Display::new(&canvas, cx);
                    canvas.graph.results.execution_session_id =
                        ExecutionSessionId::new(uuid::Uuid::new_v4());
                    assert!(!reopened.matches(&canvas, cx));
                    assert!(!reopened.same_context(&canvas));
                    canvas
                })
            })
            .unwrap();
        });
    }
}
