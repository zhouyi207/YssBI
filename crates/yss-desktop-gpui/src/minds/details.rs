//! Forms mount in the existing Details panel; captured targets guard delayed callbacks.
use super::MindCanvas;
use gpui::{Context, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Textarea,
    menu::{DropdownMenu, PopupMenuItem},
};
use yss_project_model::{file::FileVersion, mind::MindEdit};

#[derive(Clone)]
struct TopicTarget {
    id: String,
    version: FileVersion,
}
#[derive(Clone, Copy)]
enum TopicAction {
    Apply,
    DiscardInput,
    AddChild,
    AddSibling,
    Up,
    Down,
    Collapse,
    Delete,
}

impl MindCanvas {
    pub fn render_details(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut view = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.snapshot.path.name().to_owned()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("思维导图"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.path().to_owned()),
            );
        let Some(topic) = self.selected_topic().cloned() else {
            return view
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.selected.is_empty() {
                            "选择主题以编辑内容与分支".into()
                        } else {
                            format!("已选择 {} 个主题", self.selected.len())
                        }),
                )
                .into_any_element();
        };
        let Some(input) = self.ensure_buffer(&topic.id, window, cx) else {
            return view.into_any_element();
        };
        let target = TopicTarget {
            id: topic.id.clone(),
            version: self.snapshot.version.clone(),
        };
        let busy = self.busy();
        let root = topic.parent_id.is_none();
        view = view
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("主题内容"),
            )
            .child(Textarea::new(&input).small().disabled(busy))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.topic_button(
                        "apply-topic-content",
                        "应用内容",
                        TopicAction::Apply,
                        &target,
                        cx,
                    ))
                    .child(self.topic_button(
                        "discard-topic-input",
                        "恢复内容",
                        TopicAction::DiscardInput,
                        &target,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.topic_button(
                        "add-child-topic",
                        "添加子主题",
                        TopicAction::AddChild,
                        &target,
                        cx,
                    ))
                    .child(
                        self.topic_button(
                            "add-sibling-topic",
                            "添加同级",
                            TopicAction::AddSibling,
                            &target,
                            cx,
                        )
                        .disabled(busy || root),
                    ),
            );
        if !root {
            view = view
                .child(self.parent_picker(&topic, target.clone(), cx))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.topic_button("topic-up", "上移", TopicAction::Up, &target, cx))
                        .child(self.topic_button(
                            "topic-down",
                            "下移",
                            TopicAction::Down,
                            &target,
                            cx,
                        )),
                );
        }
        let collapse_label = if self.collapsed.contains(&topic.id) {
            "展开分支"
        } else {
            "折叠分支"
        };
        view.child(
            div()
                .flex()
                .gap_2()
                .child(self.topic_button(
                    "topic-collapse",
                    collapse_label,
                    TopicAction::Collapse,
                    &target,
                    cx,
                ))
                .child(
                    self.topic_button("delete-topic", "删除分支", TopicAction::Delete, &target, cx)
                        .disabled(busy || root),
                ),
        )
        .when_some(topic.reference, |view, reference| {
            view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("引用：{}", reference_label(&reference))),
            )
        })
        .into_any_element()
    }
    fn accepts_topic(&self, target: &TopicTarget) -> bool {
        !self.busy()
            && self.snapshot.version == target.version
            && self.selected.len() == 1
            && self.selected.contains(&target.id)
    }
    fn topic_button(
        &self,
        id: &'static str,
        label: &'static str,
        action: TopicAction,
        target: &TopicTarget,
        cx: &mut Context<Self>,
    ) -> Button {
        let target = target.clone();
        Button::new(id)
            .small()
            .ghost()
            .label(label)
            .disabled(self.busy())
            .on_click(cx.listener(move |view, _, window, cx| {
                if !view.accepts_topic(&target) {
                    return;
                }
                match action {
                    TopicAction::Apply => view.apply_edits(vec![], window, cx),
                    TopicAction::DiscardInput => view.discard_input(&target.id, window, cx),
                    TopicAction::AddChild => view.add_topic(false, window, cx),
                    TopicAction::AddSibling => view.add_topic(true, window, cx),
                    TopicAction::Up => view.reorder_topic(-1, window, cx),
                    TopicAction::Down => view.reorder_topic(1, window, cx),
                    TopicAction::Collapse => view.toggle_branch(target.id.clone(), cx),
                    TopicAction::Delete => view.delete_topics(window, cx),
                }
            }))
    }
    fn parent_picker(
        &self,
        topic: &yss_project_model::mind::MindNode,
        target: TopicTarget,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let parent_name = topic
            .parent_id
            .as_ref()
            .and_then(|id| {
                self.snapshot
                    .content
                    .nodes
                    .iter()
                    .find(|node| &node.id == id)
            })
            .map(|node| node.content.lines().next().unwrap_or("主题").to_owned())
            .unwrap_or_else(|| "主题".into());
        let owner = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("父主题"),
            )
            .child(
                Button::new("mind-parent")
                    .small()
                    .ghost()
                    .label(parent_name)
                    .disabled(self.busy())
                    .dropdown_menu(move |mut menu, _, cx| {
                        let candidates = owner
                            .update(cx, |view, _| {
                                if view.accepts_topic(&target) {
                                    view.parent_choices(&target.id)
                                } else {
                                    vec![]
                                }
                            })
                            .unwrap_or_default();
                        for (parent_id, title) in candidates {
                            let owner = owner.clone();
                            let target = target.clone();
                            menu = menu.item(PopupMenuItem::new(title).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.accepts_topic(&target) {
                                            view.apply_edits(
                                                vec![MindEdit::MoveNode {
                                                    node_id: target.id.clone(),
                                                    parent_id: parent_id.clone(),
                                                    before_id: None,
                                                }],
                                                window,
                                                cx,
                                            );
                                        }
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .into_any_element()
    }
    fn parent_choices(&self, id: &str) -> Vec<(String, String)> {
        // Filtering a picker is disposable presentation; Project validates the actual move.
        let mut children = std::collections::HashMap::<&str, Vec<&str>>::new();
        for node in &self.snapshot.content.nodes {
            if let Some(parent) = node.parent_id.as_deref() {
                children.entry(parent).or_default().push(&node.id);
            }
        }
        let mut descendants = std::collections::HashSet::new();
        let mut pending = vec![id];
        while let Some(node) = pending.pop() {
            descendants.insert(node);
            if let Some(group) = children.get(node) {
                pending.extend(group);
            }
        }
        self.snapshot
            .content
            .nodes
            .iter()
            .filter(|node| !descendants.contains(node.id.as_str()))
            .map(|node| {
                (
                    node.id.clone(),
                    node.content
                        .lines()
                        .next()
                        .filter(|text| !text.is_empty())
                        .unwrap_or("未命名主题")
                        .to_owned(),
                )
            })
            .collect()
    }
}
fn reference_label(reference: &yss_project_model::mind::MindReference) -> String {
    use yss_project_model::mind::MindReference;
    match reference {
        MindReference::Resource { path } => path.clone(),
        MindReference::GraphNode { path, node_id } => format!("{} · {}", path.as_str(), node_id),
        MindReference::Database { database_id } => database_id.clone(),
    }
}
