//! Structural intents and the selection that follows their successful receipt.
use super::MindCanvas;
use gpui::{Context, Window};
use yss_project_model::mind::{MindEdit, MindNode};

pub(super) struct TopicSelection {
    previous: String,
    next: String,
    expand: Option<String>,
}

impl MindCanvas {
    pub(super) fn selection_after(&self, edits: &[MindEdit]) -> Option<TopicSelection> {
        let selected = self.selected_topic()?;
        let (next, expand) = match edits {
            [MindEdit::AddNode { node }] => (node.id.clone(), node.parent_id.clone()),
            [MindEdit::RemoveNodes { node_ids }] if node_ids.contains(&selected.id) => {
                (selected.parent_id.clone()?, None)
            }
            _ => return None,
        };
        Some(TopicSelection {
            previous: selected.id.clone(),
            next,
            expand,
        })
    }

    pub(super) fn follow_selection(&mut self, selection: Option<TopicSelection>) {
        let Some(selection) = selection else { return };
        if self.selected.len() != 1 || !self.selected.contains(&selection.previous) {
            return;
        }
        // A snapshot replaces the geometry, so it also ends an in-flight gesture.
        // Do this before selection changes to avoid restoring its captured selection.
        self.cancel_gesture();
        self.selected.clear();
        self.selected.insert(selection.next);
        if let Some(parent) = selection.expand {
            self.collapsed.remove(&parent);
        }
    }

    pub(super) fn add_topic(&mut self, sibling: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(topic) = self.selected_topic() else {
            return;
        };
        let parent_id = if sibling {
            let Some(parent) = &topic.parent_id else {
                return;
            };
            parent.clone()
        } else {
            topic.id.clone()
        };
        let node = MindNode {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: Some(parent_id),
            content: crate::text::translate("documents.newTopic"),
            reference: None,
        };
        self.apply_edits(vec![MindEdit::AddNode { node }], window, cx);
    }

    pub(super) fn delete_topics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let node_ids = self
            .selected
            .iter()
            .filter(|id| *id != &self.snapshot.content.root_id)
            .cloned()
            .collect::<Vec<_>>();
        if !node_ids.is_empty() {
            self.apply_edits(vec![MindEdit::RemoveNodes { node_ids }], window, cx);
        }
    }

    pub(super) fn sibling_edit(&self, down: bool) -> Option<MindEdit> {
        let topic = self.selected_topic()?;
        let parent = topic.parent_id.as_ref()?;
        let mut siblings = self
            .snapshot
            .content
            .nodes
            .iter()
            .filter(|node| node.parent_id.as_ref() == Some(parent));
        let mut previous: Option<&MindNode> = None;
        while let Some(sibling) = siblings.next() {
            if sibling.id == topic.id {
                let before_id = if down {
                    siblings.next()?;
                    siblings.next().map(|node| node.id.clone())
                } else {
                    Some(previous?.id.clone())
                };
                return Some(MindEdit::MoveNode {
                    node_id: topic.id.clone(),
                    parent_id: parent.clone(),
                    before_id,
                });
            }
            previous = Some(sibling);
        }
        None
    }
}
