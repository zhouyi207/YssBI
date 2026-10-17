//! Capture control drafts once; successful typed edits acknowledge only their own inputs.
use super::*;
use gpui_kit::EntityId;
use yss_project::GraphEditVersion;
use yss_ui::lifecycle::LogReleased;

#[derive(Clone)]
pub(in crate::canvas) struct PortEdit {
    pub address: PortAddress,
    pub value: Value,
    input: EntityId,
}

impl GraphCanvas {
    pub(in crate::canvas) fn rebind_moved_port_inputs(
        &mut self,
        base: GraphEditVersion,
        previous: &crate::canvas::ports::details::Details,
    ) {
        let version = self.graph.editing.version;
        if version.session_id != base.session_id {
            return;
        }
        for (address, field) in &mut self.port_inputs.fields {
            if !field.dirty || field.version != base {
                continue;
            }
            let before = previous.port(address);
            let after = self
                .port_details
                .as_ref()
                .and_then(|ports| ports.port(address));
            // A verified rename may advance the input's basis only while its source is unchanged.
            if let (Some(before), Some(after)) = (before, after)
                && eligible(before) == Some(field.kind)
                && eligible(after) == Some(field.kind)
                && before.input == after.input
            {
                field.version = version;
            }
        }
    }

    pub(in crate::canvas) fn commit_port_inputs_before(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        show: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> bool {
        if !self.has_dirty_port_inputs() {
            return false;
        }
        if let Some(task) = self.submit_command(GraphCommand::CommitPortInputs, None, None, cx) {
            let (ready, settled) = tokio::sync::oneshot::channel();
            let mut ready = Some(ready);
            let observation = cx.observe(&cx.entity(), move |view, _, _| {
                if !view.busy
                    && view.refresh_task.is_none()
                    && !view.refresh_pending
                    && let Some(ready) = ready.take()
                {
                    let _ = ready.send(view.graph.editing.version);
                }
            });
            window.focus(&self.focus, cx);
            cx.spawn_in(window, async move |view, cx| {
                if let Some(version) = task.await
                    && settled.await.ok() == Some(version)
                {
                    view.update_in(cx, |view, window, cx| {
                        if view.can_edit()
                            && view.refresh_task.is_none()
                            && !view.refresh_pending
                            && view.graph.editing.version == version
                            && view.gesture.is_none()
                            && view.focus.contains_focused(window, cx)
                        {
                            show(view, window, cx);
                        }
                    })
                    .log_released();
                }
                drop(observation);
            })
            .detach();
        }
        true
    }

    pub(in crate::canvas) fn has_dirty_port_inputs(&self) -> bool {
        self.port_inputs.fields.values().any(|field| field.dirty)
    }

    pub(in crate::canvas) fn commit_blurred_port_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.can_edit()
            && self.gesture.is_none()
            && self.has_dirty_port_inputs()
            && self
                .port_inputs
                .fields
                .values()
                .all(|field| !field.focused(window, cx))
        {
            self.submit(GraphCommand::CommitPortInputs, None, cx);
        }
    }

    pub(in crate::canvas) fn prepare_port_edits(
        &mut self,
        version: Option<GraphEditVersion>,
        cx: &mut Context<Self>,
    ) -> Option<Vec<PortEdit>> {
        let version = version.unwrap_or(self.graph.editing.version);
        let mut edits = Vec::new();
        for (address, field) in &mut self.port_inputs.fields {
            if !field.dirty {
                continue;
            }
            let current = self
                .port_details
                .as_ref()
                .and_then(|ports| ports.port(address));
            if version != self.graph.editing.version
                || field.version.session_id != version.session_id
                || !current.is_some_and(|port| {
                    eligible(port) == Some(field.kind) && port.input == field.base
                })
            {
                field.error = Some(InputError::Conflict);
                self.notifications
                    .error(yss_ui::text::t("native.canvas.commandFailed"), cx);
                cx.notify();
                return None;
            }
            let text = field.input.read(cx).value();
            let value = if field.kind == SemanticType::Text {
                Ok(Value::String(text.to_string()))
            } else {
                yss_ui::input::number(&text)
            };
            let value = match value {
                Ok(value) => value,
                Err(error) => {
                    field.error = Some(InputError::Number);
                    self.notifications.error(error, cx);
                    cx.notify();
                    return None;
                }
            };
            edits.push(PortEdit {
                address: address.clone(),
                value,
                input: field.input.entity_id(),
            });
        }
        for edit in &edits {
            self.port_inputs.fields.get_mut(&edit.address)?.pending = Some(edit.value.clone());
        }
        Some(edits)
    }

    pub(in crate::canvas) fn cancel_port_edits(&mut self) {
        for field in self.port_inputs.fields.values_mut() {
            field.pending = None;
        }
    }

    pub(in crate::canvas) fn accept_port_edits(&mut self, edits: &[PortEdit]) {
        for edit in edits {
            if let Some(field) = self.port_inputs.fields.get_mut(&edit.address)
                && field.input.entity_id() == edit.input
                && field.pending.as_ref() == Some(&edit.value)
            {
                field.dirty = false;
                field.pending = None;
                field.error = None;
                field.version = self.graph.editing.version;
                field.base = self
                    .port_details
                    .as_ref()
                    .and_then(|ports| ports.port(&edit.address))
                    .and_then(|port| port.input.clone());
            }
        }
    }
}
