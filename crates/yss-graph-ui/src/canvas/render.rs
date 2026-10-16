use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::{
    Bounds, Context, IntoElement, MouseButton, Pixels, Render, Window, canvas, div, point,
    prelude::*, px,
};

use super::{Gesture, GraphCanvas, commands::*, geometry};

impl Render for GraphCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let layout_owner = owner.clone();
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
            .and_then(|pending| pending.feedback(self.bounds.get().size, cx));
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
        let focused_node = self.port_inputs.focused_node(window, cx);
        let layer = self.connection_layer.borrow();
        let visible_nodes = projection
            .nodes
            .iter()
            .zip(layer.nodes())
            .filter_map(|(node, geometry)| {
                let rect = geometry
                    .layout
                    .bounds(geometry::position(node, &self.preview));
                let top_left = offset + rect.origin * zoom;
                (focused_node == Some(node.node_id)
                    || (top_left.x + rect.size.width * zoom >= px(0.)
                        && top_left.y + rect.size.height * zoom >= px(0.)
                        && (visible.width == px(0.) || top_left.x <= visible.width)
                        && (visible.height == px(0.) || top_left.y <= visible.height)))
                    .then_some(node)
            })
            .collect::<Vec<_>>();
        drop(layer);
        self.prepare_port_inputs(&visible_nodes, window, cx);

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
            .bg(cx.theme().table)
            .text_color(cx.theme().foreground)
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
            .when(self.hovered_connection.is_some(), |view| {
                view.cursor_pointer()
            })
            .on_hover(cx.listener(|view, hovered, _, cx| {
                if !hovered && view.hovered_connection.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_move(
                cx.listener(|view, event: &gpui_kit::MouseMoveEvent, _, cx| {
                    if view.gesture.is_none() {
                        view.hover_connection(event.position, cx);
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
                        Self::register_gesture_events(owner, window);
                        let mut layer = connection_layer.borrow_mut();
                        layer.paint(
                            bounds,
                            (offset, zoom),
                            &preview,
                            super::connections::Interaction {
                                presentation: &presentation,
                                selected: &selected_connections,
                                hovered: hovered_connection,
                                replacements: replacements.as_ref(),
                            },
                            window,
                            cx,
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
            .children(
                visible_nodes
                    .into_iter()
                    .filter_map(|node| self.render_node(node, cx)),
            )
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
            .when_some(self.selection_bounds(), |view, rect| {
                view.child(
                    div()
                        .absolute()
                        .left(rect.origin.x)
                        .top(rect.origin.y)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .border_1()
                        .border_color(cx.theme().primary)
                        .bg(cx.theme().primary.opacity(0.1)),
                )
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
