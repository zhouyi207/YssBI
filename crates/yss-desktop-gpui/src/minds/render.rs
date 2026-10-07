use super::layout::TopicPlacement;
use super::{
    CancelMindGesture, DeleteTopics, FitMind, FitTopics, MindCanvas, SaveMind, SelectTopics,
};
use crate::appearance;
use gpui::{
    AppContext, Context, IntoElement, MouseButton, PathBuilder, Render, Window, canvas, div, point,
    prelude::*, px, rgb,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    text::{TextView, TextViewState},
};
use std::collections::HashMap;

impl Render for MindCanvas {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let zoom = self.zoom;
        let offset = self.offset;
        let bounds = self.bounds.clone();
        let owner = cx.entity().downgrade();
        let anchors = self
            .layout
            .nodes
            .iter()
            .map(|node| (node.index, node.bounds))
            .collect::<HashMap<_, _>>();
        let edges = self
            .layout
            .nodes
            .iter()
            .filter_map(|node| {
                let parent = anchors.get(&node.parent?)?;
                Some((
                    point(
                        parent.origin.x + parent.size.width,
                        parent.origin.y + parent.size.height / 2.,
                    ),
                    point(
                        node.bounds.origin.x,
                        node.bounds.origin.y + node.bounds.size.height / 2.,
                    ),
                ))
            })
            .collect::<Vec<_>>();
        let viewport = self.bounds.get().size;
        let placements = self
            .layout
            .nodes
            .iter()
            .filter(|node| {
                let top = offset + node.bounds.origin * zoom;
                top.x + node.bounds.size.width * zoom >= px(0.)
                    && top.y + node.bounds.size.height * zoom >= px(0.)
                    && (viewport.width == px(0.) || top.x <= viewport.width)
                    && (viewport.height == px(0.) || top.y <= viewport.height)
            })
            .cloned()
            .collect::<Vec<_>>();
        let nodes = placements
            .into_iter()
            .map(|node| self.render_topic(node, cx))
            .collect::<Vec<_>>();
        div()
            .id("mind-canvas")
            .key_context("MindCanvas")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(appearance::CANVAS))
            .on_action(cx.listener(|view, _: &SaveMind, window, cx| view.save(window, cx)))
            .on_action(cx.listener(|view, _: &SelectTopics, _, cx| {
                view.selected = view
                    .layout
                    .nodes
                    .iter()
                    .map(|node| view.snapshot.content.nodes[node.index].id.clone())
                    .collect();
                view.changed(cx);
            }))
            .on_action(
                cx.listener(|view, _: &DeleteTopics, window, cx| view.delete_topics(window, cx)),
            )
            .on_action(cx.listener(|view, _: &CancelMindGesture, _, cx| {
                if view.gesture.is_some() {
                    view.cancel_gesture();
                } else {
                    view.selected.clear();
                }
                view.changed(cx);
            }))
            .on_action(cx.listener(|view, _: &FitMind, _, cx| view.fit(false, cx)))
            .on_action(cx.listener(|view, _: &FitTopics, _, cx| view.fit(true, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::begin_pane))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::begin_pane))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::begin_pane))
            .on_mouse_move(cx.listener(Self::pointer_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::pointer_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::pointer_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::pointer_up))
            .on_scroll_wheel(cx.listener(Self::zoom_at_pointer))
            .child(
                canvas(
                    move |rect, _, cx| {
                        bounds.set(rect);
                        let _ = owner.update(cx, |view, cx| {
                            if view.fit_pending {
                                view.fit(false, cx);
                            }
                        });
                    },
                    move |rect, _, window, _| {
                        let mut path = PathBuilder::stroke(px(1.5));
                        for (a, b) in &edges {
                            let a = rect.origin + offset + *a * zoom;
                            let b = rect.origin + offset + *b * zoom;
                            let bend = (b.x - a.x) / 2.;
                            path.move_to(a);
                            path.cubic_bezier_to(b, point(a.x + bend, a.y), point(b.x - bend, b.y));
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, rgb(0x647c9d));
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
                        .border_color(cx.theme().primary)
                        .bg(gpui::rgba((appearance::BLUE << 8) | 0x18)),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .left_3()
                        .right_3()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().muted)
                        .text_color(cx.theme().danger)
                        .text_xs()
                        .child(error),
                )
            })
    }
}

impl MindCanvas {
    fn render_topic(
        &mut self,
        placement: TopicPlacement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let topic = &self.snapshot.content.nodes[placement.index];
        let id = topic.id.clone();
        let content = &topic.content;
        let label = if let Some((previous, state)) = self.labels.get_mut(&id) {
            if previous != content {
                state.update(cx, |state, cx| state.set_text(content, cx));
                previous.clone_from(content);
            }
            state.clone()
        } else {
            let state = cx.new(|cx| TextViewState::markdown(content, cx).selectable(false));
            self.labels
                .insert(id.clone(), (content.clone(), state.clone()));
            state
        };
        let origin = self.offset + placement.bounds.origin * self.zoom;
        let selected = self.selected.contains(&id);
        let collapsed = self.collapsed.contains(&id);
        let click_id = id.clone();
        let color = [
            appearance::BLUE,
            0x6eb4bf,
            appearance::GREEN,
            appearance::AMBER,
            0xb477cf,
        ][placement.depth % 5];
        div()
            .id(gpui::SharedString::from(format!("mind-topic-{id}")))
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(placement.bounds.size.width * self.zoom)
            .h(placement.bounds.size.height * self.zoom)
            .p(px(12. * self.zoom))
            .text_size(px(13. * self.zoom))
            .rounded(px(6. * self.zoom))
            .bg(if selected {
                rgb(appearance::SELECTED)
            } else {
                rgb(appearance::SURFACE)
            })
            .border_1()
            .border_color(if selected {
                cx.theme().primary
            } else {
                rgb(color).into()
            })
            .overflow_hidden()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| {
                    view.begin_topic(click_id.clone(), event, window, cx)
                }),
            )
            .child(TextView::new(&label))
            .when(placement.children > 0, |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom_1()
                        .right_1()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new(gpui::SharedString::from(format!("collapse-topic-{id}")))
                                .xsmall()
                                .ghost()
                                .label(if collapsed {
                                    format!("+{}", placement.children)
                                } else {
                                    "−".into()
                                })
                                .tooltip(if collapsed {
                                    "展开分支"
                                } else {
                                    "折叠分支"
                                })
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.focus_canvas(window, cx);
                                    view.toggle_branch(id.clone(), cx)
                                })),
                        ),
                )
            })
            .into_any_element()
    }
}
