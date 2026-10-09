//! Panel shortcuts and graph statistics follow the root DockArea's visible editor regions.
use super::{Workbench, layout::columns, menus::WorkbenchPanel};
use crate::canvas::GraphCanvas;
use gpui::{Context, Entity, IntoElement, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Selectable, Sizable,
    button::{Button, ButtonVariants},
    dock::DockPlacement,
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;

impl Workbench {
    pub(super) fn observe_graph_status(
        &mut self,
        canvas: &Entity<GraphCanvas>,
        cx: &mut Context<Self>,
    ) {
        let mut status = GraphStatus::read(canvas.read(cx));
        self.subscriptions
            .push(cx.observe(canvas, move |_, canvas, cx| {
                let next = GraphStatus::read(canvas.read(cx));
                // Only changes to the displayed values redraw the workbench chrome.
                if next != status {
                    status = next;
                    cx.notify();
                }
            }));
    }

    pub(super) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dock = self.dock.read(cx);
        let conversation_zoomed = columns::conversation_zoomed(dock);
        let left_width = if self.project.is_some()
            && dock.is_dock_open(DockPlacement::Left)
            && (!dock.is_zoomed() || conversation_zoomed)
        {
            dock.dock_size(DockPlacement::Left).unwrap_or_default()
        } else {
            px(0.)
        };
        let conversation_width =
            if self.project.is_some() && columns::conversation_edge(dock).is_some() {
                self.dock_renderer.conversation_width()
            } else {
                px(0.)
            };
        let content_left = left_width + conversation_width;
        let right_width = (self.project.is_some()
            && dock.is_dock_open(DockPlacement::Right)
            && !dock.is_zoomed())
        .then(|| dock.dock_size(DockPlacement::Right))
        .flatten();
        let graph_status = self
            .project
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
                let canvas = panel.view().downcast::<GraphCanvas>().ok()?;
                Some(GraphStatus::read(canvas.read(cx)))
            });
        div()
            .h(px(28.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_t_1()
            .border_color(cx.theme().status_bar_border)
            .bg(cx.theme().status_bar)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .when(content_left > px(0.), |view| {
                view.child(
                    div()
                        .w(content_left)
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .when(left_width > px(0.), |view| {
                            view.child(
                                div()
                                    .when(conversation_width > px(0.), |view| view.w(left_width))
                                    .pl_2()
                                    .child(self.dock_button(
                                        "status-left",
                                        IconName::PanelLeft,
                                        crate::text::t("native.workbench.toggleLeftSidebar"),
                                        DockPlacement::Left,
                                        cx,
                                    )),
                            )
                        })
                        .child(self.conversation_window_button(cx)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .pr_2()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .when(!conversation_zoomed, |view| {
                        view.child(self.panel_button(
                            "status-problems",
                            IconName::TriangleAlert,
                            crate::text::t("detail.sections.diagnostics"),
                            WorkbenchPanel::Problems,
                            cx,
                        ))
                        .child(self.panel_button(
                            "status-output",
                            IconName::SquareTerminal,
                            crate::text::t("native.workbench.runOutput"),
                            WorkbenchPanel::Output,
                            cx,
                        ))
                        .child(self.panel_button(
                            "status-results",
                            IconName::Table,
                            crate::text::t("native.workbench.runResults"),
                            WorkbenchPanel::Results,
                            cx,
                        ))
                        .child(self.panel_button(
                            "status-logs",
                            IconName::FileText,
                            crate::text::t("log.title"),
                            WorkbenchPanel::Logs,
                            cx,
                        ))
                    })
                    .child(div().flex_1().min_w_0())
                    .when(right_width.is_none(), |view| {
                        view.child(self.status_controls(
                            left_width == px(0.),
                            content_left == px(0.),
                            cx,
                        ))
                    })
                    .when_some(graph_status, |view, status| view.child(status.render())),
            )
            .when_some(right_width, |view, width| {
                view.child(
                    div()
                        .w(width)
                        .flex_shrink_0()
                        .pr_2()
                        .flex()
                        .justify_end()
                        .child(self.status_controls(
                            left_width == px(0.),
                            content_left == px(0.),
                            cx,
                        )),
                )
            })
    }

    fn status_controls(
        &self,
        show_left: bool,
        show_conversation: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .when(show_left, |view| {
                view.child(self.dock_button(
                    "status-left",
                    IconName::PanelLeft,
                    crate::text::t("native.workbench.toggleLeftSidebar"),
                    DockPlacement::Left,
                    cx,
                ))
            })
            .child(self.dock_button(
                "status-bottom",
                IconName::PanelBottom,
                crate::text::t("native.workbench.toggleBottomPanel"),
                DockPlacement::Bottom,
                cx,
            ))
            .child(div().w(px(1.)).h_3().mx_2().bg(cx.theme().border))
            .child(self.panel_button(
                "status-details",
                IconName::Inspector,
                crate::text::t("native.workbench.properties"),
                WorkbenchPanel::Details,
                cx,
            ))
            .when(show_conversation, |view| {
                view.child(self.conversation_window_button(cx))
            })
            .child(self.dock_button(
                "status-right",
                IconName::PanelRight,
                crate::text::t("native.workbench.toggleRightSidebar"),
                DockPlacement::Right,
                cx,
            ))
    }

    fn conversation_window_button(&self, cx: &mut Context<Self>) -> Button {
        let open = self.project.is_some() && self.conversation_window_open(cx);
        let label = if self.project.is_none() {
            "panel.assistant"
        } else if open {
            "bottomBar.closeConversation"
        } else {
            "bottomBar.openConversation"
        };
        Button::new("status-assistant")
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(gpui_kit_assets::IconName::MessageSquareText).size_3())
            .tooltip(crate::text::translate(label))
            .selected(open)
            .disabled(
                self.assistant_reopen || self.is_closing(cx) || self.dock.read(cx).is_locked(),
            )
            .on_click(cx.listener(|view, _, window, cx| {
                view.toggle_conversation_window(window, cx);
            }))
    }

    fn panel_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: &'static str,
        panel: WorkbenchPanel,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(icon).size_3())
            .tooltip(tooltip)
            .selected(!self.dock.read(cx).is_zoomed() && self.panel_is_displayed(panel, cx))
            .disabled(self.project.is_none() || self.is_closing(cx))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.toggle_panel(panel, window, cx);
            }))
    }

    fn dock_button(
        &self,
        id: &'static str,
        icon: IconName,
        tooltip: &'static str,
        placement: DockPlacement,
        cx: &mut Context<Self>,
    ) -> Button {
        let dock = self.dock.read(cx);
        Button::new(id)
            .xsmall()
            .compact()
            .ghost()
            .icon(Icon::new(icon).size_3())
            .tooltip(tooltip)
            .selected(
                dock.is_dock_open(placement)
                    && (!dock.is_zoomed()
                        || placement == DockPlacement::Left && columns::conversation_zoomed(dock)),
            )
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct GraphStatus {
    nodes: usize,
    connections: usize,
    offset_x: i32,
    offset_y: i32,
    zoom_percent: u16,
    busy: bool,
    run_status: &'static str,
}

impl GraphStatus {
    fn read(canvas: &GraphCanvas) -> Self {
        let offset = canvas.viewport_offset();
        Self {
            nodes: canvas.graph.projection.nodes.len(),
            connections: canvas.graph.projection.connections.len(),
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
                crate::text::format(
                    "native.workbench.nodeCount",
                    &[("value0", self.nodes.to_string())],
                ),
            ))
            .child(status_item(
                "status-graph-connections",
                IconName::Cable,
                self.connections.to_string(),
                crate::text::format(
                    "native.workbench.edgeCount",
                    &[("value0", self.connections.to_string())],
                ),
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
    div()
        .id(id)
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
}
