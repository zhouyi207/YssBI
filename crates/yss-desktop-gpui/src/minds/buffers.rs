//! Topic composition is transient and remains tied to its captured file version.
use super::MindCanvas;
use gpui::{AppContext, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, TextareaState};
use yss_project_model::{file::FileVersion, mind::MindEdit};

pub(super) struct TopicBuffer {
    pub input: Entity<TextareaState>,
    pub version: FileVersion,
    pub original: String,
    pub dirty: bool,
    _subscription: Subscription,
}
impl MindCanvas {
    pub(super) fn discard_input(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(node) = self
            .snapshot
            .content
            .nodes
            .iter()
            .find(|node| node.id == id)
        else {
            return;
        };
        if let Some(buffer) = self.buffers.get_mut(id) {
            buffer.input.update(cx, |input, cx| {
                input.set_value(node.content.clone(), window, cx)
            });
            buffer.original = node.content.clone();
            buffer.version = self.snapshot.version.clone();
            buffer.dirty = false;
            self.error = None;
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
            .snapshot
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
                version: self.snapshot.version.clone(),
                _subscription: subscription,
            },
        );
        Some(input)
    }

    pub(super) fn pending_edits(&mut self, cx: &mut Context<Self>) -> Option<Vec<MindEdit>> {
        if self
            .buffers
            .values()
            .any(|buffer| buffer.dirty && buffer.version != self.snapshot.version)
        {
            self.error =
                Some("主题已在其他位置修改。当前输入已保留，请核对后重新打开或放弃输入。".into());
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

    pub(super) fn install_snapshot(
        &mut self,
        snapshot: yss_project::minds::MindSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.snapshot = snapshot;
        let nodes = &self.snapshot.content.nodes;
        let by_id = nodes
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
                buffer.version = self.snapshot.version.clone();
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
