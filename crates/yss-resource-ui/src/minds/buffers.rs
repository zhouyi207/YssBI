//! Topic composition is transient and remains tied to its captured file version.
use super::MindCanvas;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::{AppContext, Context, Entity, Subscription, Window};
use yss_project_model::{file::FileVersion, mind::MindEdit};

pub(super) struct TopicBuffer {
    pub input: Entity<TextareaState>,
    pub version: FileVersion,
    pub original: String,
    pub dirty: bool,
    generation: u64,
    _subscription: Subscription,
}
impl MindCanvas {
    pub(super) fn discard_input(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.file.snapshot();
        let Some(node) = snapshot.content.nodes.iter().find(|node| node.id == id) else {
            return;
        };
        if let Some(buffer) = self.buffers.get_mut(id) {
            buffer.input.update(cx, |input, cx| {
                input.set_value(node.content.clone(), window, cx)
            });
            buffer.original = node.content.clone();
            buffer.version = snapshot.version.clone();
            buffer.dirty = false;
            self.file.clear_error();
            self.changed(cx);
        }
    }
    pub(super) fn ensure_buffer(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TextareaState>> {
        if let Some(buffer) = self.buffers.get(id) {
            return Some(buffer.input.clone());
        }
        let node = self
            .file
            .snapshot()
            .content
            .nodes
            .iter()
            .find(|node| node.id == id)?;
        let original = node.content.clone();
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(4, 12)
                .default_value(original.clone())
        });
        let key = id.to_owned();
        let subscription = cx.subscribe(&input, move |view, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(buffer) = view.buffers.get_mut(&key) {
                    buffer.generation += 1;
                    buffer.dirty = input.read(cx).value().as_ref() != buffer.original.as_str();
                }
                view.changed(cx);
            }
        });
        self.buffers.insert(
            id.into(),
            TopicBuffer {
                input: input.clone(),
                original,
                dirty: false,
                generation: 0,
                version: self.file.snapshot().version.clone(),
                _subscription: subscription,
            },
        );
        Some(input)
    }

    pub(super) fn submitted_inputs(&self) -> Vec<(String, u64)> {
        self.buffers
            .iter()
            .filter(|(_, buffer)| buffer.dirty)
            .map(|(id, buffer)| (id.clone(), buffer.generation))
            .collect()
    }

    pub(super) fn acknowledge_inputs(&mut self, submitted: &[(String, u64)]) {
        for (id, generation) in submitted {
            if let Some(buffer) = self.buffers.get_mut(id)
                && buffer.generation == *generation
            {
                // A deleted topic has no content in the receipt to acknowledge its draft.
                // Retain a draft typed after submission, including on a removed topic.
                buffer.dirty = false;
            }
        }
    }

    pub(super) fn pending_edits(&mut self, cx: &mut Context<Self>) -> Option<Vec<MindEdit>> {
        if self
            .buffers
            .values()
            .any(|buffer| buffer.dirty && buffer.version != self.file.snapshot().version)
        {
            self.file
                .report(yss_ui::text::t("native.minds.topicChanged"), cx);
            self.changed(cx);
            return None;
        }
        Some(
            self.buffers
                .iter()
                .filter(|(_, buffer)| buffer.dirty)
                .map(|(id, buffer)| MindEdit::SetContent {
                    node_id: id.clone(),
                    content: buffer.input.read(cx).value().to_string(),
                })
                .collect(),
        )
    }

    /// Topic buffers follow the installed snapshot unless they hold their own draft.
    pub(super) fn reconcile_buffers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.file.snapshot();
        let by_id = snapshot
            .content
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<std::collections::HashMap<_, _>>();
        self.buffers.retain(|id, buffer| {
            let Some(node) = by_id.get(id.as_str()) else {
                return buffer.dirty;
            };
            if !buffer.dirty || buffer.input.read(cx).value().as_ref() == node.content.as_str() {
                if buffer.input.read(cx).value().as_ref() != node.content.as_str() {
                    buffer.input.update(cx, |input, cx| {
                        input.set_value(node.content.clone(), window, cx)
                    });
                }
                buffer.original = node.content.clone();
                buffer.version = snapshot.version.clone();
                buffer.dirty = false;
            }
            true
        });
        self.collapsed.retain(|id| by_id.contains_key(id.as_str()));
        self.cancel_gesture();
        self.rebuild_layout();
        self.changed(cx);
    }
}
