//! Inline controls are created for visible scalar ports; all writes use the graph edit owner.
pub(in crate::canvas) mod commit;
mod field;
mod render;

use crate::canvas::{GraphCanvas, GraphCommand};
use field::{Field, InputError};
use gpui::{Context, EntityId, Window, prelude::*};
use gpui_component::input::{InputEvent, InputState};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use yss_data_contract::SemanticType;
use yss_graph_document::PortAddress;
use yss_graph_editor::{
    EditorGraphMutation,
    projection::{EditorNodeModel, EditorPortModel, EditorProjectionModel},
};

#[derive(Default)]
pub(in crate::canvas) struct Inputs {
    projection: Option<Arc<EditorProjectionModel>>,
    fields: BTreeMap<PortAddress, Field>,
}

fn eligible(port: &EditorPortModel) -> Option<SemanticType> {
    (port.connections.current == 0)
        .then(|| super::scalar_input_type(port))
        .flatten()
}

impl GraphCanvas {
    pub(in crate::canvas) fn prepare_port_inputs(
        &mut self,
        nodes: &[&EditorNodeModel],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let version = self.graph.editing.version;
        let projection = &self.graph.projection;
        if self
            .port_inputs
            .projection
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(previous, projection))
        {
            let ports = projection
                .nodes
                .iter()
                .flat_map(|node| node.ports.iter())
                .map(|port| (&port.address, port))
                .collect::<BTreeMap<_, _>>();
            self.port_inputs.fields.retain(|address, field| {
                let Some(port) = ports
                    .get(address)
                    .filter(|port| eligible(port) == Some(field.kind))
                else {
                    return false;
                };
                field.install(port, version, window, cx);
                true
            });
            self.port_inputs.projection = Some(projection.clone());
        }
        let mut visible = BTreeSet::new();
        for port in nodes.iter().flat_map(|node| node.ports.iter()) {
            let Some(kind) = eligible(port).filter(|kind| *kind != SemanticType::Binary) else {
                continue;
            };
            visible.insert(port.address.clone());
            if let Some(field) = self.port_inputs.fields.get_mut(&port.address) {
                if kind == SemanticType::Text
                    && field.input.read(cx).presentation().placeholder().as_ref()
                        != crate::text::t("conversion.text")
                {
                    crate::text::input_placeholder(&field.input, "conversion.text", window, cx);
                    field.measure(window, cx);
                }
                continue;
            }
            let input = cx.new(|cx| {
                let input = InputState::new(window, cx).default_value(field::text(port, kind));
                if kind == SemanticType::Numeric {
                    input.validate(field::numeric_draft)
                } else {
                    input.placeholder(crate::text::translate("conversion.text"))
                }
            });
            let address = port.address.clone();
            let subscription =
                cx.subscribe_in(&input, window, move |view, input, event, window, cx| {
                    view.port_input_event(&address, input.entity_id(), event, window, cx);
                });
            let mut field = Field {
                kind,
                input,
                version,
                dirty: false,
                pending: None,
                skip_blur: false,
                width: gpui::px(28.),
                error: None,
                _subscription: subscription,
            };
            field.measure(window, cx);
            self.port_inputs.fields.insert(port.address.clone(), field);
        }
        self.port_inputs.fields.retain(|address, field| {
            visible.contains(address) || field.dirty || field.focused(window, cx)
        });
    }

    fn port_input_event(
        &mut self,
        address: &PortAddress,
        id: EntityId,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self
            .port_inputs
            .fields
            .get_mut(address)
            .filter(|field| field.input.entity_id() == id)
        else {
            return;
        };
        match event {
            InputEvent::Focus => {
                if !field.dirty {
                    field.version = self.graph.editing.version;
                }
                field.skip_blur = false;
                self.cancel_gesture();
            }
            InputEvent::Change => {
                let was_dirty = field.dirty;
                if !field.dirty {
                    field.version = self.graph.editing.version;
                }
                field.dirty = true;
                field.pending = None;
                field.error = None;
                field.measure(window, cx);
                if !was_dirty {
                    cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
                }
            }
            InputEvent::PressEnter { .. } => {
                window.focus(&self.focus, cx);
                self.cancel_gesture();
                self.commit_port_input(address, window, cx);
            }
            InputEvent::Blur => {
                if std::mem::take(&mut field.skip_blur) {
                    return;
                }
                self.commit_port_input(address, window, cx);
            }
        }
        cx.notify();
    }

    fn commit_port_input(
        &mut self,
        address: &PortAddress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(field) = self.port_inputs.fields.get_mut(address) {
            let has_literal = self
                .graph
                .projection
                .nodes
                .iter()
                .flat_map(|node| node.ports.iter())
                .find(|port| port.address == *address)
                .and_then(|port| port.input.as_ref())
                .is_some_and(|input| input.literal_override.is_some());
            if !field.dirty && has_literal {
                return;
            }
            field.dirty = true;
        }
        self.commit_blurred_port_inputs(window, cx);
    }

    fn cancel_port_input(
        &mut self,
        address: &PortAddress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(port) = self
            .graph
            .projection
            .nodes
            .iter()
            .flat_map(|node| node.ports.iter())
            .find(|port| port.address == *address)
        else {
            return;
        };
        if let Some(field) = self.port_inputs.fields.get_mut(address) {
            field.restore(port, self.graph.editing.version, window, cx);
            field.skip_blur = true;
        }
        window.focus(&self.focus, cx);
        cx.emit(gpui_component::dock::PanelEvent::LayoutChanged);
        cx.notify();
    }
}
