//! Each visible Dock column owns its shortcuts and graph statistics.
use super::{Workbench, layout::columns, menus::WorkbenchPanel};
use crate::canvas::GraphCanvas;
use gpui_kit::assets::IconName;
use gpui_kit::base::TestSupportExt;
use gpui_kit::component::{
    Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    dock::DockPlacement,
    status_bar::StatusBar,
    tooltip::Tooltip,
};
use gpui_kit::{
    App, Context, Entity, IntoElement, Rems, Render, RenderOnce, Subscription, WeakEntity, Window,
    div, prelude::*,
};

pub(super) const STATUS_BAR_HEIGHT: Rems = yss_ui::appearance::HEADER_HEIGHT;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StatusColumn {
    Left,
    Conversation,
    Editor,
    Right,
}

#[derive(IntoElement)]
pub(super) struct ColumnStatusBar {
    owner: WeakEntity<Workbench>,
    column: StatusColumn,
}

impl ColumnStatusBar {
    pub(super) fn new(owner: WeakEntity<Workbench>, column: StatusColumn) -> Self {
        Self { owner, column }
    }
}

impl RenderOnce for ColumnStatusBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // Dock's renderer only constructs this component. Read Dock after its
        // render borrow has ended, when GPUI renders the child.
        let bar = self
            .owner
            .upgrade()
            .map(|owner| owner.update(cx, |owner, cx| owner.column_status_bar(self.column, cx)))
            .unwrap_or_else(StatusBar::new);
        let id = match self.column {
            StatusColumn::Left => "statusbar-left",
            StatusColumn::Conversation => "statusbar-conversation",
            StatusColumn::Editor => "statusbar-editor",
            StatusColumn::Right => "statusbar-right",
        };
        div()
            .id(id)
            .w_full()
            .min_w_0()
            .h(STATUS_BAR_HEIGHT)
            .flex_shrink_0()
            .child(bar.w_full().h_full().py_0().gap_0())
            .test_support()
    }
}

pub(super) struct GraphStatistics {
    owner: WeakEntity<Workbench>,
}

impl GraphStatistics {
    pub(super) fn new(owner: WeakEntity<Workbench>) -> Self {
        Self { owner }
    }

    fn observe_graph(&self, canvas: &Entity<GraphCanvas>, cx: &mut Context<Self>) -> Subscription {
        let mut status = GraphStatus::read(canvas.read(cx));
        cx.observe(canvas, move |view, canvas, cx| {
            let next = GraphStatus::read(canvas.read(cx));
            if next == status {
                return;
            }
            status = next;
            if view.owner.upgrade().is_some_and(|owner| {
                owner
                    .read(cx)
                    .status_graph(cx)
                    .is_some_and(|active| active == canvas)
            }) {
                cx.notify();
            }
        })
    }
}

impl Render for GraphStatistics {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Resolve shell routing through live Dock visibility; never retain selection or viewport.
        self.owner
            .upgrade()
            .and_then(|owner| owner.read(cx).status_graph(cx))
            .map(|canvas| {
                GraphStatus::read(canvas.read(cx))
                    .render()
                    .into_any_element()
            })
            .unwrap_or_else(|| div().into_any_element())
    }
}

impl Workbench {
    pub(super) fn observe_graph_status(
        &mut self,
        canvas: &Entity<GraphCanvas>,
        cx: &mut Context<Self>,
    ) {
        let observation = self
            .graph_statistics
            .update(cx, |view, cx| view.observe_graph(canvas, cx));
        self.subscriptions.push(observation);
        self.subscriptions.push(observe_graph_commands(canvas, cx));
    }

    fn column_status_bar(&self, column: StatusColumn, cx: &mut Context<Self>) -> StatusBar {
        let dock = self.dock.read(cx);
        let has_project = self.project.is_some();
        let conversation_zoomed = columns::conversation_zoomed(dock);
        let left_visible = has_project && columns::dock_shown(dock, DockPlacement::Left);
        let conversation_visible =
            has_project && (conversation_zoomed || columns::conversation_edge(dock).is_some());
        let right_visible = has_project && columns::dock_shown(dock, DockPlacement::Right);

        let bar = StatusBar::new();
        match column {
            StatusColumn::Left => bar.left(
                div()
                    .flex()
                    .items_center()
                    .child(self.left_status_button(cx))
                    .when(!conversation_visible, |row| {
                        row.child(self.conversation_window_button(cx))
                    }),
            ),
            StatusColumn::Conversation => bar
                .left(
                    div()
                        .flex()
                        .items_center()
                        .when(!left_visible, |row| row.child(self.left_status_button(cx)))
                        .child(self.conversation_window_button(cx)),
                )
                .when(conversation_zoomed && !right_visible, |bar| {
                    bar.right(self.right_status_button(cx))
                }),
            StatusColumn::Editor => bar
                .left(
                    div()
                        .flex()
                        .items_center()
                        .when(!left_visible && !conversation_visible, |row| {
                            row.child(self.left_status_button(cx))
                                .child(self.conversation_window_button(cx))
                        })
                        .child(self.panel_button(
                            "status-problems",
                            IconName::TriangleAlert,
                            crate::text::t("detail.sections.diagnostics"),
                            WorkbenchPanel::Problems,
                            cx,
                        ))
                        .child(self.panel_button(
                            "status-output",
                            IconName::SquareTerminal,
                            crate::text::t("panel.output"),
                            WorkbenchPanel::Output,
                            cx,
                        ))
                        .child(self.panel_button(
                            "status-results",
                            IconName::Table,
                            crate::text::t("panel.results"),
                            WorkbenchPanel::Results,
                            cx,
                        ))
                        .child(self.panel_button(
                            "status-logs",
                            IconName::FileText,
                            crate::text::t("log.title"),
                            WorkbenchPanel::Logs,
                            cx,
                        )),
                )
                .right(
                    div()
                        .flex()
                        .items_center()
                        .when(!right_visible, |row| {
                            row.child(self.right_status_button(cx))
                        })
                        .child(self.graph_statistics.clone()),
                ),
            StatusColumn::Right => bar.right(self.right_status_button(cx)),
        }
    }

    fn status_graph(&self, cx: &App) -> Option<Entity<GraphCanvas>> {
        let dock = self.dock.read(cx);
        self.project
            .as_ref()
            .and_then(|_| self.active_editor_panel(cx))
            .and_then(|panel| {
                let id = panel.panel_id(cx);
                let placement = self.displayed_panel_placement(id, cx)?;
                if let Some(zoomed) = dock.zoomed_group()
                    && dock.layout(placement)?.find_panel_node(id) != Some(zoomed)
                {
                    return None;
                }
                panel.view().downcast::<GraphCanvas>().ok()
            })
    }

    fn left_status_button(&self, cx: &mut Context<Self>) -> Button {
        self.dock_button(
            "status-left",
            IconName::PanelLeft,
            crate::text::t("native.workbench.toggleLeftSidebar"),
            DockPlacement::Left,
            cx,
        )
    }

    fn right_status_button(&self, cx: &mut Context<Self>) -> Button {
        self.dock_button(
            "status-right",
            IconName::PanelRight,
            crate::text::t("native.workbench.toggleRightSidebar"),
            DockPlacement::Right,
            cx,
        )
    }

    fn conversation_window_button(&self, cx: &mut Context<Self>) -> Button {
        let open = self.project.is_some() && self.conversation_window_open(cx);
        let label = if open {
            "bottomBar.closeConversation"
        } else {
            "bottomBar.openConversation"
        };
        Button::new("status-assistant")
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(gpui_kit::assets::IconName::MessageSquareText).size_3())
            .tooltip(crate::text::translate(label))
            .accessibility_label(crate::text::translate(label))
            .selected(open)
            .disabled(
                self.project.is_none()
                    || self.assistant.read(cx).is_reopening()
                    || self.is_closing(cx)
                    || self.dock.read(cx).is_locked(),
            )
            .on_click(cx.listener(|view, _, window, cx| {
                view.toggle_conversation_window(window, cx);
            }))
    }

    fn panel_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: std::borrow::Cow<'static, str>,
        panel: WorkbenchPanel,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(icon).size_3())
            .accessibility_label(tooltip.clone())
            .tooltip(tooltip)
            .selected(
                self.project.is_some()
                    && !self.dock.read(cx).is_zoomed()
                    && self.panel_is_displayed(panel, cx),
            )
            .disabled(self.project.is_none() || self.is_closing(cx))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.toggle_panel(panel, window, cx);
            }))
    }

    fn dock_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: std::borrow::Cow<'static, str>,
        placement: DockPlacement,
        cx: &mut Context<Self>,
    ) -> Button {
        let dock = self.dock.read(cx);
        Button::new(id)
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(icon).size_3())
            .accessibility_label(tooltip.clone())
            .tooltip(tooltip)
            .selected(self.project.is_some() && columns::dock_shown(dock, placement))
            .disabled(self.project.is_none() || !dock.has_dock(placement) || self.is_closing(cx))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.toggle_dock_region(placement, window, cx);
            }))
    }

    pub(super) fn toggle_dock_region(
        &self,
        placement: DockPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_closing(cx) || self.project.is_none() {
            return;
        }
        self.dock.update(cx, |dock, cx| {
            if dock.is_zoomed()
                && !(placement == DockPlacement::Left && columns::conversation_zoomed(dock))
            {
                dock.set_zoomed_out(window, cx);
                if !dock.is_dock_open(placement) {
                    dock.toggle_dock(placement, window, cx);
                }
            } else {
                dock.toggle_dock(placement, window, cx);
            }
        });
    }
}

fn observe_graph_commands<T: 'static>(
    canvas: &Entity<GraphCanvas>,
    cx: &mut Context<T>,
) -> Subscription {
    let mut status = graph_command_status(canvas.read(cx), cx);
    cx.observe(canvas, move |_, canvas, cx| {
        let next = graph_command_status(canvas.read(cx), cx);
        // Viewport and counts belong to GraphStatistics, not workbench chrome.
        if next != status {
            status = next;
            cx.notify();
        }
    })
}

fn graph_command_status(canvas: &GraphCanvas, cx: &App) -> (bool, bool, [bool; 6]) {
    use crate::canvas::GraphEditCommand::*;
    (
        canvas.busy(),
        canvas.dirty(cx),
        [Undo, Redo, Cut, Copy, Paste, Delete]
            .map(|command| canvas.is_edit_command_enabled(command)),
    )
}

#[derive(Clone, PartialEq, Eq)]
struct GraphStatus {
    nodes: usize,
    connections: usize,
    offset_x: i32,
    offset_y: i32,
    zoom_percent: u16,
    busy: bool,
    run_status: std::borrow::Cow<'static, str>,
}

impl GraphStatus {
    fn read(canvas: &GraphCanvas) -> Self {
        let offset = canvas.viewport_offset();
        Self {
            nodes: canvas.graph().projection().nodes.len(),
            connections: canvas.graph().projection().connections.len(),
            offset_x: f32::from(offset.x).round() as i32,
            offset_y: f32::from(offset.y).round() as i32,
            zoom_percent: (canvas.zoom() * 100.).round() as u16,
            busy: canvas.busy(),
            run_status: canvas.run_status(),
        }
    }

    fn render(self) -> impl IntoElement {
        div()
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .when(self.busy, |view| {
                view.child(status_item(
                    "status-graph-busy",
                    IconName::LoaderCircle,
                    String::new(),
                    crate::text::t("native.databases.submitting").into(),
                ))
            })
            .when(!self.run_status.is_empty(), |view| {
                view.child(status_item(
                    "status-graph-run",
                    IconName::Activity,
                    String::new(),
                    self.run_status.into(),
                ))
            })
            .child(status_item(
                "status-graph-nodes",
                IconName::Workflow,
                self.nodes.to_string(),
                crate::text::plural("counts.nodes", self.nodes),
            ))
            .child(status_item(
                "status-graph-connections",
                IconName::Cable,
                self.connections.to_string(),
                crate::text::plural("counts.connections", self.connections),
            ))
            .child(status_item(
                "status-graph-position",
                IconName::Move,
                format!("X {} Y {}", self.offset_x, self.offset_y),
                crate::text::t("native.workbench.viewportPosition").into(),
            ))
            .child(status_item(
                "status-graph-zoom",
                IconName::ZoomIn,
                format!("{}%", self.zoom_percent),
                crate::text::t("native.workbench.viewportZoom").into(),
            ))
    }
}

fn status_item(
    id: &'static str,
    icon: IconName,
    label: String,
    tooltip: String,
) -> impl IntoElement {
    let label = gpui_kit::SharedString::from(label);
    div()
        .id(id)
        .aria_label(label.clone())
        .h_full()
        .px_1()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_1()
        .whitespace_nowrap()
        .child(Icon::new(icon).size_3())
        .when(!label.is_empty(), |view| view.child(label))
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .test_support()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, InputEvent as _, TestAppContext, WindowOptions};
    use std::{cell::Cell, rc::Rc, sync::Arc};
    use yss_application::{graph::open::OpenGraphRequest, session::ApplicationState};
    use yss_graph_document::{
        DocumentNode, GraphResourceKind, GraphResourcePath, NodeId, NodePosition,
    };

    struct CommandChrome {
        _observation: Subscription,
    }

    struct Readout {
        canvas: Entity<GraphCanvas>,
        _observation: Subscription,
    }

    impl Render for Readout {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(self.canvas.clone())
                .child(
                    div()
                        .h(STATUS_BAR_HEIGHT)
                        .child(GraphStatus::read(self.canvas.read(cx)).render()),
                )
        }
    }

    #[gpui_kit::test]
    fn coordinates_update_readout_without_command_notifications(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = GraphResourcePath::new("events/Status.yssbi-event").unwrap();
        let mut data = yss_project_model::ProjectData::new();
        let mut resource =
            yss_project_model::GraphResourceDocument::new("Status", GraphResourceKind::EventGraph);
        let node = NodeId::new();
        Arc::make_mut(&mut resource.document).nodes.insert(
            node,
            DocumentNode {
                id: node,
                node_type: "yssbi.dataframe.source.get".parse().unwrap(),
                position: NodePosition { x: 200., y: 300. },
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
        let locale = crate::text::locale();
        let graph = yss_graph_ui::OpenedGraph::from_open(
            application
                .open_graph(OpenGraphRequest::new(project.clone(), path, locale))
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
        let services = Arc::new(yss_graph_ui::GraphServices::new(
            tasks,
            Arc::new(yss_ui::layout_store::LayoutStore::new(
                directory.path(),
                runtime.handle().clone(),
            )),
        ));
        cx.update(|cx| {
            gpui_kit::init(cx);
            yss_ui::init(cx);
        });
        let (window, readout) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| {
                    let canvas = cx.new(|cx| {
                        GraphCanvas::new(services, graph, catalog, locale.into(), window, cx)
                    });
                    Readout {
                        _observation: cx.observe(&canvas, |_, _, cx| cx.notify()),
                        canvas,
                    }
                })
            })
            .unwrap()
        });
        let canvas = cx.update(|cx| readout.read(cx).canvas.clone());
        let initial = cx.update(|cx| GraphStatus::read(canvas.read(cx)));
        // Exercise the exact subscription used by Workbench, independently of the readout.
        let commands = cx.update(|cx| {
            cx.new(|cx| CommandChrome {
                _observation: observe_graph_commands(&canvas, cx),
            })
        });
        let notifications = Rc::new(Cell::new(0));
        let count = notifications.clone();
        let _observation =
            cx.update(|cx| cx.observe(&commands, move |_, _| count.set(count.get() + 1)));
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("status-graph-nodes").label(), Some("1"));
            assert_eq!(window.find("status-graph-connections").label(), Some("0"));
            let size = window.viewport_size();
            let start = gpui_kit::point(size.width / 2., size.height / 2.);
            let end = start + gpui_kit::point(gpui_kit::px(80.), gpui_kit::px(40.));
            window.dispatch_event(
                gpui_kit::MouseDownEvent {
                    button: gpui_kit::MouseButton::Middle,
                    position: start,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                gpui_kit::MouseMoveEvent {
                    position: end,
                    pressed_button: Some(gpui_kit::MouseButton::Middle),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            window.dispatch_event(
                gpui_kit::MouseUpEvent {
                    button: gpui_kit::MouseButton::Middle,
                    position: end,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        })
        .unwrap();
        assert_eq!(notifications.replace(0), 0);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("status-graph-position").label(),
                Some(format!("X {} Y {}", initial.offset_x + 80, initial.offset_y + 40).as_str()),
            );
            assert_eq!(
                window.find("status-graph-zoom").label(),
                Some(format!("{}%", initial.zoom_percent).as_str()),
            );
            canvas.update(cx, |canvas, cx| assert!(canvas.prepare_save(cx).is_some()));
        })
        .unwrap();
        assert!(notifications.replace(0) > 0, "busy disables commands");
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("status-graph-busy").visible());
            canvas.update(cx, |canvas, cx| canvas.cancel_prepared_save(cx));
        })
        .unwrap();
        assert!(notifications.replace(0) > 0, "idle restores commands");
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("status-graph-busy").is_none());
            canvas.update(cx, |_, cx| cx.notify());
        })
        .unwrap();
        assert_eq!(notifications.get(), 0, "unchanged status is silent");
    }
}
