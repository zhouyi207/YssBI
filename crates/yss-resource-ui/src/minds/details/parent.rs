//! Derive eligible parents only when the picker opens, then validate its captured target.
use super::{MindCanvas, TopicTarget};
use gpui_kit::component::{
    Disableable,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Context, IntoElement};
use yss_project_model::mind::{MindEdit, MindNode};
use yss_ui::inspector;

impl MindCanvas {
    pub(super) fn parent_picker(
        &self,
        topic: &MindNode,
        target: TopicTarget,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let current = topic.parent_id.clone();
        let parent_name = self
            .file
            .snapshot()
            .content
            .nodes
            .iter()
            .find(|node| Some(&node.id) == current.as_ref())
            .map(topic_title)
            .unwrap_or_else(|| yss_ui::text::translate("native.minds.unnamedTopic"));
        let owner = cx.entity().downgrade();
        inspector::choice_trigger("mind-parent", parent_name)
            .disabled(!self.file.editable())
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
                    let target = target.clone();
                    let checked = current.as_ref() == Some(&parent_id);
                    menu = menu.item(PopupMenuItem::new(title).checked(checked).on_click(
                        inspector::owner_command(
                            owner.clone(),
                            move |view: &mut MindCanvas, window, cx| {
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
                            },
                        ),
                    ));
                }
                menu
            })
    }

    fn parent_choices(&self, id: &str) -> Vec<(String, String)> {
        // This index only filters presentation. Project validates the actual tree edit.
        let mut children = std::collections::HashMap::<&str, Vec<&str>>::new();
        for node in &self.file.snapshot().content.nodes {
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
        self.file
            .snapshot()
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
            || yss_ui::text::translate("native.minds.unnamedTopic"),
            str::to_owned,
        )
}
