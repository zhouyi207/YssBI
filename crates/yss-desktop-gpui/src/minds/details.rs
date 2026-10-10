//! Forms mount in the existing Details panel; captured targets guard delayed callbacks.
mod parent;

use super::MindCanvas;
use gpui::{Context, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Textarea,
};
use yss_project_model::file::FileVersion;

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
                    .child(crate::text::t("documents.minds")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.path().to_owned()),
            )
            .when_some(self.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .id("mind-details-error")
                        .role(gpui::accesskit::Role::Alert)
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
            });
        let Some(id) = self.selected_topic().map(|topic| topic.id.clone()) else {
            return view
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.selected.is_empty() {
                            crate::text::t("native.minds.selectTopicHint").into()
                        } else {
                            crate::text::format(
                                "native.minds.selectedTopics",
                                &[("value0", self.selected.len().to_string())],
                            )
                        }),
                )
                .into_any_element();
        };
        let Some(input) = self.ensure_buffer(&id, window, cx) else {
            return view.into_any_element();
        };
        let Some(topic) = self.selected_topic() else {
            return view.into_any_element();
        };
        let target = TopicTarget {
            id,
            version: self.snapshot.version.clone(),
        };
        let busy = self.busy();
        let root = topic.parent_id.is_none();
        view = view
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.minds.topicContent")),
            )
            .child(Textarea::new(&input).small().disabled(busy))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.topic_button(
                        "apply-topic-content",
                        crate::text::t("native.minds.applyContent"),
                        TopicAction::Apply,
                        &target,
                        cx,
                    ))
                    .child(self.topic_button(
                        "discard-topic-input",
                        crate::text::t("native.minds.restoreContent"),
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
                        crate::text::t("documents.addChild"),
                        TopicAction::AddChild,
                        &target,
                        cx,
                    ))
                    .child(
                        self.topic_button(
                            "add-sibling-topic",
                            crate::text::t("native.minds.addSibling"),
                            TopicAction::AddSibling,
                            &target,
                            cx,
                        )
                        .disabled(busy || root),
                    ),
            );
        if !root {
            view = view
                .child(self.parent_picker(topic, target.clone(), cx))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            self.topic_button(
                                "topic-up",
                                crate::text::t("conversion.moveUp"),
                                TopicAction::Up,
                                &target,
                                cx,
                            )
                            .disabled(busy || self.sibling_edit(false).is_none()),
                        )
                        .child(
                            self.topic_button(
                                "topic-down",
                                crate::text::t("conversion.moveDown"),
                                TopicAction::Down,
                                &target,
                                cx,
                            )
                            .disabled(busy || self.sibling_edit(true).is_none()),
                        ),
                );
        }
        let collapse_label = if self.collapsed.contains(&topic.id) {
            crate::text::t("documents.expand")
        } else {
            crate::text::t("documents.collapse")
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
                    self.topic_button(
                        "delete-topic",
                        crate::text::t("documents.deleteBranch"),
                        TopicAction::Delete,
                        &target,
                        cx,
                    )
                    .disabled(busy || root),
                ),
        )
        .when_some(topic.reference.as_ref(), |view, reference| {
            view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::format(
                        "native.minds.reference",
                        &[("value0", reference_label(reference))],
                    )),
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
                    TopicAction::Up | TopicAction::Down => {
                        if let Some(edit) = view.sibling_edit(matches!(action, TopicAction::Down)) {
                            view.apply_edits(vec![edit], window, cx);
                        }
                    }
                    TopicAction::Collapse => view.toggle_branch(target.id.clone(), cx),
                    TopicAction::Delete => view.delete_topics(window, cx),
                }
            }))
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
