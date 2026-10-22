use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::{
    AnyElement, Bounds, Context, IntoElement, MouseButton, Pixels, Render, SharedString, Window,
    canvas, div, point, prelude::*, px,
};
use yss_graph_editor::projection::EditorNodeModel;

use super::{CanvasTarget, Gesture, GraphCanvas, commands::*, ports, style::CanvasStyle};

type TooltipBuilder = Box<dyn Fn(&mut Window, &mut gpui_kit::App) -> gpui_kit::AnyView>;

impl Render for GraphCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let layout_owner = owner.clone();
        let bounds_cell = self.bounds.clone();
        let feedback = self
            .pending_connection(cx)
            .and_then(|pending| pending.feedback(self.bounds.get().size, cx));
        let label = self
            .hovered_connection()
            .map(|id| self.presentation.connection(id).label());
        let projection = self.graph.projection.clone();
        let focused = self.port_inputs.focused_node(window, cx);
        let visible_nodes = self.visible_nodes(&projection, focused);
        let controls = self.controls_visible();
        if controls {
            self.prepare_port_inputs(&visible_nodes, window, cx);
        } else {
            let editing = visible_nodes
                .iter()
                .copied()
                .filter(|node| focused == Some(node.node_id))
                .collect::<Vec<_>>();
            self.prepare_port_inputs(&editing, window, cx);
        }
        self.prepare_node_texts(&visible_nodes, window);
        let inputs = self.render_port_inputs(&visible_nodes, controls, focused, cx);
        let hover = self.render_hover_target(cx);
        let located = self.render_located_port();
        let activity = self.activity_visible().then(|| self.activity.clone());
        let cursor = match (&self.gesture, &self.hover) {
            (Some(Gesture::Pan { moved: true, .. }), _) => gpui_kit::CursorStyle::ClosedHand,
            (Some(Gesture::Connection(_)), _) | (None, Some(CanvasTarget::Port(_))) => {
                gpui_kit::CursorStyle::Crosshair
            }
            (None, Some(CanvasTarget::Connection(_))) => gpui_kit::CursorStyle::PointingHand,
            _ => gpui_kit::CursorStyle::Arrow,
        };

        let surface = div()
            .id("graph-canvas")
            .role(gpui_kit::accesskit::Role::Pane)
            .aria_label(self.title_text(cx))
            .key_context("GraphCanvas")
            .track_focus(&self.focus)
            .w_full()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .bg(CanvasStyle::new(cx).surface)
            .text_color(cx.theme().foreground)
            .cursor(cursor)
            .on_action(
                cx.listener(|view, _: &SaveGraph, _, cx| view.submit(GraphCommand::Save, None, cx)),
            )
            .on_action(cx.listener(|view, _: &UndoGraph, _, cx| {
                view.edit(super::GraphEditCommand::Undo, cx);
            }))
            .on_action(cx.listener(|view, _: &RedoGraph, _, cx| {
                view.edit(super::GraphEditCommand::Redo, cx);
            }))
            .on_action(cx.listener(|view, _: &DeleteSelection, _, cx| {
                view.edit(super::GraphEditCommand::Delete, cx);
            }))
            .on_action(
                cx.listener(|view, _: &DuplicateSelection, _, cx| view.duplicate_selection(cx)),
            )
            .on_action(
                cx.listener(|view, _: &gpui_kit::component::input::Copy, _, cx| {
                    view.edit(super::GraphEditCommand::Copy, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &gpui_kit::component::input::Cut, _, cx| {
                    view.edit(super::GraphEditCommand::Cut, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &gpui_kit::component::input::Paste, _, cx| {
                    view.edit(super::GraphEditCommand::Paste, cx)
                }),
            )
            .on_action(cx.listener(|view, _: &SelectAll, _, cx| {
                view.cancel_gesture();
                view.located_port = None;
                view.selected_connections.clear();
                view.connection_click = None;
                view.selected = view
                    .graph
                    .projection
                    .nodes
                    .iter()
                    .filter(|node| !node.capabilities.managed)
                    .map(|node| node.node_id)
                    .collect();
                view.emit_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &CancelGesture, window, cx| {
                view.cancel_gesture();
                view.palette = None;
                view.located_port = None;
                view.connection_click = None;
                window.focus(&view.focus, cx);
                view.commit_blurred_port_inputs(window, cx);
                view.emit_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &OpenNodePalette, window, cx| {
                view.open_node_palette(window, cx);
            }))
            .on_action(cx.listener(|view, _: &FrameGraph, window, cx| {
                view.reveal_graph(window, cx);
            }))
            .on_action(cx.listener(|view, _: &FrameSelection, window, cx| {
                view.frame_selection(window, cx);
            }))
            .on_action(cx.listener(|view, _: &RunWholeGraph, _, cx| {
                view.run_graph(yss_application::graph::run::RunDemand::Default, cx);
            }))
            .on_action(cx.listener(|view, _: &CancelRun, _, cx| view.cancel_run(cx)))
            .on_action(cx.listener(|view, _: &RefreshRunState, _, cx| view.resync_execution(cx)))
            .on_action(cx.listener(|view, _: &RunCurrentNode, _, cx| {
                view.run_selected(
                    yss_graph_execution::plan::NodeExecutionMode::CurrentInputs,
                    cx,
                );
            }))
            .on_action(cx.listener(|view, _: &RunToNode, _, cx| {
                view.run_selected(
                    yss_graph_execution::plan::NodeExecutionMode::Dependencies,
                    cx,
                );
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_pane))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::begin_pane))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::begin_pane))
            .on_drop(cx.listener(Self::drop_activity))
            .on_drop(cx.listener(Self::drop_constant))
            .on_hover(cx.listener(|view, hovered, _, cx| {
                if !hovered && view.hover.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_move(
                cx.listener(|view, event: &gpui_kit::MouseMoveEvent, _, cx| {
                    if view.gesture.is_none() {
                        view.update_hover(event.position, cx);
                    }
                }),
            )
            .on_pinch(cx.listener(Self::pinch))
            .on_scroll_wheel(cx.listener(Self::zoom_at_pointer))
            .child(
                canvas(
                    move |bounds, window, _| {
                        if bounds_cell.replace(bounds).size != bounds.size {
                            // Culling used the previous layout; settle the newly visible controls.
                            window.on_next_frame(move |_, cx| {
                                if let Some(owner) = layout_owner.upgrade() {
                                    owner.update(cx, |_, cx| cx.notify());
                                }
                            });
                        }
                    },
                    move |bounds, _, window, cx| {
                        Self::register_gesture_events(owner.clone(), window);
                        if let Some(view) = owner.upgrade() {
                            view.read(cx).paint_scene(bounds, window, cx);
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(activity)
            .children(inputs)
            .children(hover)
            .children(located)
            .children(feedback)
            .when(
                projection.nodes.is_empty() && self.palette.is_none(),
                |view| {
                    view.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(yss_ui::text::t("native.canvas.emptyGraph")),
                            )
                            .child(
                                Button::new("add-first-node")
                                    .label(yss_ui::text::t("native.canvas.addNode"))
                                    .icon(IconName::Plus)
                                    .disabled(!self.can_edit())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.open_node_palette(window, cx);
                                    })),
                            ),
                    )
                },
            )
            .when_some(label, |view, label| {
                view.tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(gpui_kit::SharedString::from(
                        label.clone(),
                    ))
                    .build(window, cx)
                })
            })
            .when_some(self.context_menu.as_ref(), |view, menu| {
                view.child(menu.render())
            })
            .child(
                div().absolute().bottom_2().right_2().child(
                    Button::new("reset-view")
                        .small()
                        .ghost()
                        .icon(IconName::Frame)
                        .accessibility_label(yss_ui::text::t("native.canvas.resetView"))
                        .tooltip(yss_ui::text::t("native.canvas.resetViewShortcut"))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.reveal_graph(window, cx);
                        })),
                ),
            )
            .when(
                self.resource_move.is_some() && !self.refresh_failed,
                |view| {
                    view.child(
                        div()
                            .absolute()
                            .top_2()
                            .left_2()
                            .right_2()
                            .p_2()
                            .rounded_md()
                            .bg(cx.theme().background)
                            .text_sm()
                            .child(yss_ui::text::t("native.canvas.loadingRenamedGraph")),
                    )
                },
            )
            .when(self.palette.is_some(), |view| {
                view.child(self.render_palette(window, cx))
            });
        div().size_full().flex().flex_col().child(surface)
    }
}

impl GraphCanvas {
    /// Nodes on screen in the last laid-out frame, plus any node whose inline field
    /// holds focus, so its draft stays alive while scrolled away.
    fn visible_nodes<'a>(
        &self,
        projection: &'a yss_graph_editor::projection::EditorProjectionModel,
        focused: Option<yss_graph_document::NodeId>,
    ) -> Vec<&'a EditorNodeModel> {
        let bounds = self.bounds.get();
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return projection
                .nodes
                .iter()
                .filter(|node| focused == Some(node.node_id))
                .collect();
        }
        projection
            .nodes
            .iter()
            .zip(self.connection_layer.nodes())
            .filter(|(node, geometry)| {
                focused == Some(node.node_id)
                    || self
                        .node_screen_rect(node, geometry, bounds.origin)
                        .intersects(&bounds)
            })
            .map(|(node, _)| node)
            .collect()
    }

    fn render_port_inputs(
        &self,
        nodes: &[&EditorNodeModel],
        controls: bool,
        focused: Option<yss_graph_document::NodeId>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let origin = point(px(0.), px(0.));
        let mut elements = Vec::new();
        for node in nodes {
            if !controls && focused != Some(node.node_id) {
                continue;
            }
            let Some(geometry) = self.connection_layer.node(node.node_id) else {
                continue;
            };
            if geometry.layout.compact {
                continue;
            }
            let rect = self.node_screen_rect(node, geometry, origin);
            for (row, index) in geometry.inputs.iter().enumerate() {
                let Some(port) = node.ports.get(*index) else {
                    continue;
                };
                let Some(width) = self.field_width(port) else {
                    continue;
                };
                let region = geometry.layout.port_region(false, row);
                let row = Bounds::new(
                    rect.origin + region.origin * self.zoom,
                    region.size.map(|length| length * self.zoom),
                );
                let lane = self.field_lane(rect, row, width);
                elements.extend(self.render_port_input(port, lane, cx));
            }
        }
        elements
    }

    /// An invisible element over the hovered pin or status lane that owns its tooltip.
    fn render_hover_target(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.gesture.is_some() || self.context_menu.is_some() {
            return None;
        }
        let origin = point(px(0.), px(0.));
        let (id, bounds, tooltip): (SharedString, _, TooltipBuilder) = match self.hover.as_ref()? {
            CanvasTarget::Port(address) => {
                let bounds = self.port_screen_bounds(address, origin)?;
                let owner = cx.entity().downgrade();
                let address = address.clone();
                (
                    format!("port-tooltip-{address}").into(),
                    bounds,
                    Box::new(move |_, cx| {
                        ports::details::tooltip(owner.clone(), address.clone(), cx)
                    }),
                )
            }
            CanvasTarget::Status(id) => {
                let (node, geometry) =
                    self.painted_nodes().find(|(node, _)| node.node_id == *id)?;
                let bounds = self.status_bounds(self.node_screen_rect(node, geometry, origin));
                let text = SharedString::from(self.status_tooltip(*id)?);
                (
                    format!("status-tooltip-{id}").into(),
                    bounds,
                    Box::new(move |window, cx| Tooltip::new(text.clone()).build(window, cx)),
                )
            }
            CanvasTarget::Node(_) | CanvasTarget::Connection(_) => return None,
        };
        Some(
            div()
                .id(id)
                .absolute()
                .left(bounds.left())
                .top(bounds.top())
                .w(bounds.size.width)
                .h(bounds.size.height)
                .tooltip(tooltip)
                .into_any_element(),
        )
    }

    /// Keyboard focus for a port revealed from another panel.
    fn render_located_port(&self) -> Option<AnyElement> {
        let bounds = self.port_screen_bounds(self.located_port.as_ref()?, point(px(0.), px(0.)))?;
        Some(
            div()
                .absolute()
                .left(bounds.left())
                .top(bounds.top())
                .w(bounds.size.width)
                .h(bounds.size.height)
                .track_focus(&self.port_focus)
                .into_any_element(),
        )
    }

    pub(super) fn selection_bounds(&self) -> Option<Bounds<Pixels>> {
        if let Some(Gesture::Selection { press, current, .. }) = &self.gesture {
            let a = *press - self.bounds.get().origin;
            let b = *current - self.bounds.get().origin;
            Some(Bounds::from_corners(
                point(a.x.min(b.x), a.y.min(b.y)),
                point(a.x.max(b.x), a.y.max(b.y)),
            ))
        } else {
            None
        }
    }

    fn render_palette(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = self.palette.as_ref().expect("palette is open");
        let size = self.bounds.get().size;
        let configuring = palette.view.read(cx).configuring();
        let unit = window.rem_size() / 14.;
        let width = unit * if configuring { 420. } else { 300. };
        let height = (unit * if configuring { 560. } else { 396. }).min(size.height);
        let x = palette
            .point
            .x
            .min((size.width - width).max(px(0.)))
            .max(px(0.));
        let y = palette
            .point
            .y
            .min((size.height - height).max(px(0.)))
            .max(px(0.));
        let origin = self.bounds.get().origin;
        let view = palette.view.clone();
        // A backdrop handles outside presses in the bubble phase, so child
        // dropdowns can consume their own presses beyond the form's bounds.
        gpui_kit::deferred(
            gpui_kit::anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("node-palette-backdrop")
                    .relative()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .occlude()
                    .on_any_mouse_down(cx.listener(|view, _, _, cx| {
                        if let Some(palette) = &view.palette {
                            palette.view.update(cx, |palette, cx| palette.dismiss(cx));
                        }
                        cx.stop_propagation();
                    }))
                    .child(
                        div()
                            .absolute()
                            .left(origin.x + x)
                            .top(origin.y + y)
                            .w(width.min(size.width))
                            .h(height)
                            .child(view),
                    ),
            ),
        )
        .with_priority(1)
    }
}
