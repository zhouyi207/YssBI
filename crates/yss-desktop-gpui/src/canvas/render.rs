use gpui::{
    Bounds, Context, IntoElement, MouseButton, Pixels, Render, Window, canvas, div, point,
    prelude::*, px, rgb,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Icon, IconName, Sizable};
use yss_graph_editor::{
    EditorGraphMutation,
    projection::{EditorNodeModel, EditorPortModel},
};
use yss_node_protocol::PortDirection;

use super::{Gesture, GraphCanvas, commands::*, geometry};
use crate::{appearance, assets::NativeIcon};

impl Render for GraphCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = self.zoom;
        let offset = self.offset;
        let bounds_cell = self.bounds.clone();
        let connection_layer = self.connection_layer.clone();
        let preview = self.preview.clone();
        let pending = match &self.gesture {
            Some(Gesture::Connection {
                source, current, ..
            }) => connection_layer
                .borrow()
                .port_position(source, &self.preview)
                .map(|start| (offset + start * zoom, *current)),
            _ => None,
        };
        let projection = self.graph.projection.clone();
        let visible = self.bounds.get().size;
        let nodes = projection
            .nodes
            .iter()
            .filter(|node| {
                let pos = geometry::position(node, &self.preview);
                let rect = geometry::node_bounds(node, pos);
                let top_left = offset + rect.origin * zoom;
                top_left.x + rect.size.width * zoom >= px(0.)
                    && top_left.y + rect.size.height * zoom >= px(0.)
                    && (visible.width == px(0.) || top_left.x <= visible.width)
                    && (visible.height == px(0.) || top_left.y <= visible.height)
            })
            .map(|node| self.render_node(node, cx))
            .collect::<Vec<_>>();

        let surface = div()
            .id("graph-canvas")
            .key_context("GraphCanvas")
            .track_focus(&self.focus)
            .w_full()
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .bg(rgb(appearance::CANVAS))
            .text_color(rgb(appearance::TEXT))
            .on_action(
                cx.listener(|view, _: &SaveGraph, _, cx| view.submit(GraphCommand::Save, None, cx)),
            )
            .on_action(cx.listener(|view, _: &UndoGraph, _, cx| {
                if view.graph.editing.can_undo {
                    view.submit(GraphCommand::Undo, None, cx);
                }
            }))
            .on_action(cx.listener(|view, _: &RedoGraph, _, cx| {
                if view.graph.editing.can_redo {
                    view.submit(GraphCommand::Redo, None, cx);
                }
            }))
            .on_action(cx.listener(|view, _: &DeleteSelection, _, cx| {
                let node_ids = view
                    .graph
                    .projection
                    .nodes
                    .iter()
                    .filter(|node| {
                        view.selected.contains(&node.node_id) && !node.capabilities.managed
                    })
                    .map(|node| node.node_id)
                    .collect::<Vec<_>>();
                if !node_ids.is_empty() {
                    view.submit(
                        GraphCommand::Edit(EditorGraphMutation::DeleteNodes { node_ids }),
                        None,
                        cx,
                    );
                }
            }))
            .on_action(cx.listener(|view, _: &SelectAll, _, cx| {
                view.selected = view
                    .graph
                    .projection
                    .nodes
                    .iter()
                    .map(|node| node.node_id)
                    .collect();
                view.emit_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &CancelGesture, window, cx| {
                view.cancel_gesture();
                view.palette = None;
                window.focus(&view.focus, cx);
                view.emit_selection(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &FrameGraph, _, cx| {
                view.reset_view();
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &RunWholeGraph, _, cx| {
                view.run_graph(yss_application::graph::run::RunDemand::Default, cx);
            }))
            .on_action(cx.listener(|view, _: &CancelRun, _, cx| view.cancel_run(cx)))
            .on_action(cx.listener(|view, _: &RefreshRunState, _, cx| view.resync_execution(cx)))
            .on_action(cx.listener(|view, _: &RunCurrentNode, _, cx| {
                if view.selected.len() == 1
                    && let Some(node_id) = view.selected.iter().next().copied()
                {
                    view.run_graph(
                        yss_application::graph::run::RunDemand::Node {
                            node_id,
                            mode: yss_graph_execution::plan::NodeExecutionMode::CurrentInputs,
                        },
                        cx,
                    );
                }
            }))
            .on_action(cx.listener(|view, _: &RunToNode, _, cx| {
                if view.selected.len() == 1
                    && let Some(node_id) = view.selected.iter().next().copied()
                {
                    view.run_graph(
                        yss_application::graph::run::RunDemand::Node {
                            node_id,
                            mode: yss_graph_execution::plan::NodeExecutionMode::Dependencies,
                        },
                        cx,
                    );
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_pane))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::begin_pane))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::begin_pane))
            .on_drop(
                cx.listener(|view, drag: &crate::workbench::NodeDrag, window, cx| {
                    if view.busy() {
                        return;
                    }
                    let Some(creation) = drag.creation(&view.graph.project, cx) else {
                        return;
                    };
                    view.submit(
                        GraphCommand::Edit(EditorGraphMutation::CreateNode {
                            descriptor: creation.clone(),
                            position: view.world(window.mouse_position()),
                            connect_from: None,
                            parameters: Default::default(),
                            port_counts: Default::default(),
                            user_label: None,
                        }),
                        Some(view.graph.editing.version),
                        cx,
                    );
                }),
            )
            .on_drop(cx.listener(|view, drag: &super::ConstantDrag, window, cx| {
                if view.busy
                    || view.graph.project != drag.project
                    || view.graph.projection.graph_path != drag.path
                {
                    return;
                }
                view.submit(
                    GraphCommand::Edit(EditorGraphMutation::InsertConstantReference {
                        id: drag.id,
                        position: view.world(window.mouse_position()),
                    }),
                    Some(drag.version),
                    cx,
                );
            }))
            .on_mouse_move(cx.listener(Self::pointer_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::pointer_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::pointer_up))
            .on_scroll_wheel(cx.listener(Self::zoom_at_pointer))
            .child(
                canvas(
                    move |bounds, _, _| {
                        bounds_cell.set(bounds);
                    },
                    move |bounds, _, window, _| {
                        let mut layer = connection_layer.borrow_mut();
                        layer.paint(bounds, offset, zoom, &preview, window);
                        if let Some((start, current)) = pending {
                            layer.paint_pending(bounds.origin + start, current, window);
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(nodes)
            .when_some(self.selection_bounds(), |view, rect| {
                view.child(
                    div()
                        .absolute()
                        .left(rect.origin.x)
                        .top(rect.origin.y)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .border_1()
                        .border_color(rgb(appearance::BLUE))
                        .bg(gpui::rgba((appearance::BLUE << 8) | 0x18)),
                )
            })
            .child(
                div()
                    .absolute()
                    .bottom_2()
                    .left_2()
                    .text_xs()
                    .text_color(rgb(appearance::MUTED))
                    .child(format!(
                        "{} 个节点 · {} 条连线 · {:.0}%{}{}",
                        self.graph.projection.nodes.len(),
                        self.graph.projection.connections.len(),
                        self.zoom * 100.,
                        if self.busy { " · 正在提交…" } else { "" },
                        if self.run_status().is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", self.run_status())
                        }
                    )),
            )
            .child(
                div().absolute().bottom_2().right_2().child(
                    Button::new("reset-view")
                        .small()
                        .ghost()
                        .icon(IconName::Frame)
                        .tooltip("重置视图 · Home")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.reset_view();
                            cx.notify();
                        })),
                ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .absolute()
                        .bottom_8()
                        .left_2()
                        .right_2()
                        .p_2()
                        .rounded_md()
                        .bg(gpui::rgba((appearance::RED << 8) | 0x26))
                        .text_color(rgb(appearance::RED))
                        .text_sm()
                        .child(error),
                )
            })
            .when(self.palette.is_some(), |view| {
                view.child(self.render_palette(window, cx))
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_toolbar(cx))
            .child(surface)
    }
}

impl GraphCanvas {
    fn render_node(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = node.node_id;
        let position = geometry::position(node, &self.preview);
        let origin = self.offset + point(px(position.x as f32), px(position.y as f32)) * self.zoom;
        let border = if self.selected.contains(&id) {
            appearance::BLUE
        } else if !node.diagnostics.is_empty() {
            appearance::AMBER
        } else {
            appearance::BORDER_STRONG
        };
        let inputs = node
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let outputs = node
            .ports
            .iter()
            .filter(|p| p.direction == PortDirection::Output)
            .collect::<Vec<_>>();
        div()
            .id(gpui::SharedString::from(format!("node-{id}")))
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(px(geometry::NODE_WIDTH * self.zoom))
            .h(px(geometry::node_height(node) * self.zoom))
            .rounded(px(4. * self.zoom))
            .border_1()
            .border_color(rgb(border))
            .bg(rgb(appearance::SURFACE))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| view.begin_node(id, event, window, cx)),
            )
            .child(
                div()
                    .h(px(geometry::TITLE_HEIGHT * self.zoom))
                    .px(px(12. * self.zoom))
                    .flex()
                    .items_center()
                    .gap(px(8. * self.zoom))
                    .rounded_t(px(4. * self.zoom))
                    .bg(rgb(appearance::SURFACE_RAISED))
                    .border_b_1()
                    .border_color(rgb(appearance::BORDER))
                    .text_size(px(13. * self.zoom))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(
                        Icon::new(if node.node_type.as_str().contains("source.") {
                            NativeIcon::Database
                        } else {
                            NativeIcon::Graph
                        })
                        .size(px(15. * self.zoom))
                        .text_color(rgb(appearance::BLUE)),
                    )
                    .child(
                        div().flex_1().min_w_0().truncate().child(
                            node.display
                                .user_label
                                .as_deref()
                                .unwrap_or(&node.display.title)
                                .to_owned(),
                        ),
                    ),
            )
            .children((0..inputs.len().max(outputs.len())).map(|index| {
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px((geometry::TITLE_HEIGHT
                        + index as f32 * geometry::PORT_HEIGHT)
                        * self.zoom))
                    .h(px(geometry::PORT_HEIGHT * self.zoom))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w_1_2()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .children(inputs.get(index).map(|port| self.render_port(port, cx))),
                    )
                    .child(
                        div()
                            .w_1_2()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_end()
                            .children(outputs.get(index).map(|port| self.render_port(port, cx))),
                    )
            }))
    }

    fn render_port(
        &self,
        port: &EditorPortModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let start = port.address.clone();
        let end = start.clone();
        let output = port.direction == PortDirection::Output;
        let label = port
            .display
            .instance_label
            .as_deref()
            .unwrap_or(&port.display.label)
            .to_owned();
        let color = if port.orphan {
            0xf7768e
        } else if matches!(self.gesture, Some(Gesture::Connection { .. })) {
            self.connection_candidates
                .as_ref()
                .and_then(|candidates| candidates.get(&port.address))
                .map_or(0x536079, |decision| match decision {
                    yss_graph_editor::projection::ConnectionDecision::Append => 0x9ece6a,
                    yss_graph_editor::projection::ConnectionDecision::Replace { .. } => 0xe0af68,
                    yss_graph_editor::projection::ConnectionDecision::Invalid { .. } => 0x536079,
                })
        } else {
            if output {
                appearance::GREEN
            } else {
                appearance::BLUE
            }
        };
        let dot = || {
            div()
                .size(px(10. * self.zoom))
                .flex_shrink_0()
                .rounded_full()
                .bg(rgb(color))
                .border_1()
                .border_color(rgb(appearance::CANVAS))
        };
        div()
            .id(gpui::SharedString::from(format!(
                "port-{}-{:?}",
                port.address.node_id, port.address.port
            )))
            .flex()
            .items_center()
            .gap(px(6. * self.zoom))
            .w_full()
            .min_w_0()
            .text_size(px(12. * self.zoom))
            .text_color(rgb(appearance::MUTED))
            .cursor_crosshair()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| {
                    view.begin_port(start.clone(), event, window, cx)
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _, _, cx| view.end_port(end.clone(), cx)),
            )
            .when(!output, |view| {
                view.ml(px(-5. * self.zoom))
                    .child(dot())
                    .child(div().min_w_0().flex_1().truncate().child(label.clone()))
            })
            .when(output, |view| {
                view.mr(px(-5. * self.zoom))
                    .justify_end()
                    .child(div().min_w_0().truncate().child(label))
                    .child(dot())
            })
    }

    fn selection_bounds(&self) -> Option<Bounds<Pixels>> {
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
        let width = if configuring { 420. } else { 300. };
        let height = if configuring { 560. } else { 396. };
        let height = px(height).min(size.height);
        let x = palette
            .point
            .x
            .min((size.width - px(width)).max(px(0.)))
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
        gpui::deferred(
            gpui::anchored().position(point(px(0.), px(0.))).child(
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
                            .w(px(width).min(size.width))
                            .h(height)
                            .child(view),
                    ),
            ),
        )
        .with_priority(1)
    }
}
