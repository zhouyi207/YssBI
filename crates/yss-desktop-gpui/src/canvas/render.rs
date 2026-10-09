use gpui::{
    Bounds, Context, IntoElement, MouseButton, Pixels, Render, Window, canvas, div, point,
    prelude::*, px, rgb,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Sizable};
use yss_graph_editor::EditorGraphMutation;

use super::{Gesture, GraphCanvas, commands::*, geometry};
use crate::appearance;

impl Render for GraphCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = self.zoom;
        let offset = self.offset;
        let bounds_cell = self.bounds.clone();
        let connection_layer = self.connection_layer.clone();
        let presentation = self.presentation.clone();
        let preview = self.preview.clone();
        let selected_connections = self.selected_connections.clone();
        let hovered_connection = self.hovered_connection;
        let pending = self.pending_connection(cx);
        let feedback = pending
            .as_ref()
            .and_then(|pending| pending.feedback(self.bounds.get().size));
        let replacements = pending.as_ref().map(|_| {
            self.connection_drag()
                .map(|drag| drag.replaced.clone())
                .unwrap_or_default()
        });
        let label = self
            .hovered_connection
            .map(|id| presentation.connection(id).label());
        let running_visible = connection_layer.borrow().has_visible_running(
            &presentation,
            self.bounds.get(),
            offset,
            zoom,
            &self.preview,
        );
        let activity = running_visible.then(|| self.render_connection_activity(pending.is_some()));
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
                if !view.selected_connections.is_empty() {
                    view.submit(
                        GraphCommand::Edit(EditorGraphMutation::DisconnectConnections {
                            connection_ids: view.selected_connections.iter().copied().collect(),
                        }),
                        None,
                        cx,
                    );
                    return;
                }
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
                view.located_port = None;
                view.selected_connections.clear();
                view.connection_click = None;
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
                view.located_port = None;
                view.connection_click = None;
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
                cx.listener(|view, drag: &crate::workbench::ActivityDrag, window, cx| {
                    if view.busy() {
                        return;
                    }
                    match drag.resolve(&view.graph.project, view.path(), cx) {
                        Some(crate::workbench::ActivityDrop::OpenGraph(path)) => {
                            cx.emit(super::CanvasEvent::OpenGraph(path.to_owned()));
                        }
                        Some(crate::workbench::ActivityDrop::CreateNode(creation)) => {
                            view.create_node_at(
                                creation.clone(),
                                view.world(window.mouse_position()),
                                cx,
                            );
                        }
                        None => {}
                    }
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
            .when(self.hovered_connection.is_some(), |view| {
                view.cursor_pointer()
            })
            .on_hover(cx.listener(|view, hovered, _, cx| {
                if !hovered && view.hovered_connection.take().is_some() {
                    cx.notify();
                }
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
                        layer.paint(
                            bounds,
                            offset,
                            zoom,
                            &preview,
                            super::connections::Interaction {
                                presentation: &presentation,
                                selected: &selected_connections,
                                hovered: hovered_connection,
                                replacements: replacements.as_ref(),
                            },
                            window,
                        );
                        if let Some(pending) = pending {
                            layer.paint_pending(
                                bounds.origin + pending.start,
                                bounds.origin + pending.end,
                                pending.from_input,
                                pending.color,
                                window,
                            );
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(activity)
            .children(nodes)
            .children(feedback)
            .when_some(label, |view, label| {
                view.tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(label).build(window, cx)
                })
            })
            .when_some(self.connection_menu.as_ref(), |view, menu| {
                view.child(menu.render())
            })
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
