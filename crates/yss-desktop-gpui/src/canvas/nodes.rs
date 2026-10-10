mod menu;
pub(super) mod summary;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, tooltip::Tooltip};
use gpui_kit::{Context, IntoElement, MouseButton, div, point, prelude::*, px, rgb};
use yss_graph_editor::projection::{EditorDiagnosticSeverity, EditorNodeModel};
use yss_node_protocol::PortDirection;

use super::{
    GraphCanvas,
    geometry::{self, NodeLayout},
    presentation::{NodeAppearance, State},
};
use crate::{appearance, text};

impl GraphCanvas {
    pub(super) fn render_node(
        &self,
        node: &EditorNodeModel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = node.node_id;
        let layout = NodeLayout::new(node);
        let position = geometry::position(node, &self.preview);
        let origin = self.offset + point(px(position.x as f32), px(position.y as f32)) * self.zoom;
        let display = self
            .presentation
            .nodes
            .get(&id)
            .copied()
            .unwrap_or_default();
        let selected = self.selected.contains(&id);
        let border = if selected {
            appearance::BLUE
        } else if display.state == State::Unexecuted {
            appearance::BORDER_STRONG
        } else {
            display.state.color()
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
        let primary = node
            .diagnostics
            .iter()
            .find(|d| d.blocking)
            .or_else(|| node.diagnostics.first());
        let dimmed = self
            .connection_drag()
            .is_some_and(|drag| drag.dimmed_nodes.contains(&id));
        div()
            .id(gpui_kit::SharedString::from(format!("node-{id}")))
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(px(layout.width * self.zoom))
            .h(px(layout.height * self.zoom))
            .rounded(px(4. * self.zoom))
            .border_1()
            .border_color(rgb(border))
            .when(
                matches!(display.state, State::Unexecuted | State::Stale),
                |view| view.border_dashed(),
            )
            .bg(rgb(appearance::SURFACE))
            .opacity(if dimmed { 0.35 } else { 1. })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| view.begin_node(id, event, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, event, window, cx| {
                    view.begin_node_menu(id, event, window, cx)
                }),
            )
            .when(
                matches!(display.state, State::Error | State::Valid),
                |view| {
                    view.child(
                        div()
                            .absolute()
                            .size_full()
                            .rounded(px(4. * self.zoom))
                            .bg(rgb(display.state.color()).opacity(0.07)),
                    )
                },
            )
            .when(display.state == State::Partial, |view| {
                view.child(
                    div()
                        .absolute()
                        .inset(px(3. * self.zoom))
                        .rounded(px(2. * self.zoom))
                        .border_1()
                        .border_color(rgb(appearance::GREEN)),
                )
            })
            .when(!layout.compact, |view| {
                view.child(self.render_node_header(node))
                    .child(self.render_node_parameters(id))
            })
            .when(layout.compact, |view| {
                view.child(
                    div()
                        .absolute()
                        .left(px((layout.width - 8.) / 2. * self.zoom))
                        .top(px((layout.height - 8.) / 2. * self.zoom))
                        .size(px(8. * self.zoom))
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(appearance::MUTED)),
                )
            })
            .children((0..inputs.len().max(outputs.len())).map(|index| {
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(
                        (layout.ports_top + index as f32 * layout.port_height) * self.zoom
                    ))
                    .h(px(layout.port_height * self.zoom))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w_1_2()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .children(
                                inputs
                                    .get(index)
                                    .map(|port| self.render_port(port, layout.compact, cx)),
                            ),
                    )
                    .child(
                        div()
                            .w_1_2()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_end()
                            .children(
                                outputs
                                    .get(index)
                                    .map(|port| self.render_port(port, layout.compact, cx)),
                            ),
                    )
            }))
            .child(self.render_node_badge(display, primary))
            .when(display.state != State::Error, |view| {
                view.children(primary.map(|diagnostic| {
                    let color = match diagnostic.severity {
                        EditorDiagnosticSeverity::Error => appearance::RED,
                        EditorDiagnosticSeverity::Warning => appearance::AMBER,
                        EditorDiagnosticSeverity::Information => appearance::BLUE,
                    };
                    let diagnostic = diagnostic.clone();
                    div()
                        .id("diagnostic")
                        .absolute()
                        .left(px(-5. * self.zoom))
                        .top(px(-5. * self.zoom))
                        .size(px(10. * self.zoom))
                        .rounded_full()
                        .bg(rgb(color))
                        .tooltip(move |window, cx| {
                            Tooltip::new(text::graph_diagnostic(&diagnostic)).build(window, cx)
                        })
                }))
            })
    }

    fn render_node_header(&self, node: &EditorNodeModel) -> impl IntoElement + use<> {
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
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .child(node.display.title.to_string()),
            )
            .when_some(node.display.user_label.clone(), |view, label| {
                view.child(
                    div()
                        .max_w(px(120. * self.zoom))
                        .truncate()
                        .text_size(px(11. * self.zoom))
                        .text_color(rgb(appearance::MUTED))
                        .child(label.to_string()),
                )
            })
    }

    fn render_node_parameters(&self, id: yss_graph_document::NodeId) -> impl IntoElement + use<> {
        let rows = self.node_contents.rows(id);
        div().when(!rows.is_empty(), |view| {
            view.py(px(6. * self.zoom))
                .px(px(8. * self.zoom))
                .border_b_1()
                .border_color(rgb(appearance::BORDER))
                .children(rows.iter().map(|row| {
                    div()
                        .h(px(geometry::PARAMETER_HEIGHT * self.zoom))
                        .flex()
                        .items_center()
                        .gap(px(8. * self.zoom))
                        .text_size(px(12. * self.zoom))
                        .child(div().min_w_0().flex_1().truncate().child(row.title.clone()))
                        .child(
                            div()
                                .max_w(px(130. * self.zoom))
                                .truncate()
                                .text_color(rgb(appearance::MUTED))
                                .child(row.value.clone()),
                        )
                }))
        })
    }

    fn render_node_badge(
        &self,
        display: NodeAppearance,
        diagnostic: Option<&yss_graph_editor::projection::EditorDiagnosticModel>,
    ) -> impl IntoElement + use<> {
        let icon = match display.state {
            State::Unexecuted => IconName::CircleDashed,
            State::Running => IconName::Play,
            State::Error => IconName::CircleAlert,
            State::Valid => IconName::CircleCheck,
            State::Stale => IconName::Clock,
            State::Partial => IconName::CircleDot,
        };
        let diagnostic = diagnostic
            .filter(|_| display.state == State::Error)
            .cloned();
        div()
            .id("execution-state")
            .absolute()
            .top(px(-10. * self.zoom))
            .right(px(-6. * self.zoom))
            .flex()
            .items_center()
            .gap(px(4. * self.zoom))
            .px(px(4. * self.zoom))
            .py(px(self.zoom))
            .rounded(px(4. * self.zoom))
            .border_1()
            .border_color(rgb(appearance::BORDER_STRONG))
            .bg(rgb(appearance::SURFACE))
            .text_size(px(10. * self.zoom))
            .child(
                Icon::new(icon)
                    .size(px(12. * self.zoom))
                    .text_color(rgb(display.state.color())),
            )
            .when(display.cache.total > 0, |view| {
                view.child(
                    div()
                        .text_color(rgb(display.cache.state().color()))
                        .child(format!("{}/{}", display.cache.valid, display.cache.total)),
                )
            })
            .tooltip(move |window, cx| {
                let mut lines = vec![display.state.label().into_owned()];
                if let Some(code) = display.failure {
                    lines.push(text::run_failure(code));
                }
                if let Some(diagnostic) = &diagnostic {
                    lines.push(text::graph_diagnostic(diagnostic));
                }
                if display.cache.total > 0 {
                    if display.cache.state() != display.state {
                        lines.push(display.cache.state().label().into_owned());
                    }
                    lines.push(text::format(
                        "canvas.resultCount",
                        &[
                            ("valid", display.cache.valid.to_string()),
                            ("total", display.cache.total.to_string()),
                        ],
                    ));
                }
                Tooltip::new(lines.join("\n")).build(window, cx)
            })
    }
}
