//! Derive eligible parents only when the picker opens, then validate its captured target.
use super::{MindCanvas, TopicTarget};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Context, IntoElement, div, prelude::*};
use yss_project_model::mind::{MindEdit, MindNode};

impl MindCanvas {
    pub(super) fn parent_picker(
        &self,
        topic: &MindNode,
        target: TopicTarget,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let current = topic.parent_id.clone();
        let parent_name = self
            .snapshot
            .content
            .nodes
            .iter()
            .find(|node| Some(&node.id) == current.as_ref())
            .map(topic_title)
            .unwrap_or_else(|| crate::text::translate("native.minds.unnamedTopic"));
        let owner = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::translate("documents.parent")),
            )
            .child(
                Button::new("mind-parent")
                    .small()
                    .ghost()
                    .label(parent_name)
                    .disabled(self.busy())
                    .dropdown_menu(move |menu, _, cx| {
                        let candidates = owner
                            .update(cx, |view, _| {
                                if view.accepts_topic(&target) {
                                    view.parent_choices(&target.id)
                                } else {
                                    vec![]
                                }
                            })
                            .unwrap_or_default();
                        let mut menu = menu.scrollable(true);
                        for (parent_id, title) in candidates {
                            let owner = owner.clone();
                            let target = target.clone();
                            let checked = current.as_ref() == Some(&parent_id);
                            menu = menu.item(PopupMenuItem::new(title).checked(checked).on_click(
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if !checked && view.accepts_topic(&target) {
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
        // This index only filters presentation. Project validates the actual tree edit.
        let mut children = std::collections::HashMap::<&str, Vec<&str>>::new();
        for node in &self.snapshot.content.nodes {
            if let Some(parent) = node.parent_id.as_deref() {
                children.entry(parent).or_default().push(&node.id);
            }
        }
        let mut descendants = std::collections::HashSet::new();
        let mut pending = vec![id];
        while let Some(node) = pending.pop() {
            if descendants.insert(node)
                && let Some(group) = children.get(node)
            {
                pending.extend(group);
            }
        }
        self.snapshot
            .content
            .nodes
            .iter()
            .filter(|node| !descendants.contains(node.id.as_str()))
            .map(|node| (node.id.clone(), topic_title(node)))
            .collect()
    }
}

fn topic_title(topic: &MindNode) -> String {
    topic
        .content
        .lines()
        .next()
        .filter(|text| !text.is_empty())
        .map_or_else(
            || crate::text::translate("native.minds.unnamedTopic"),
            str::to_owned,
        )
}
