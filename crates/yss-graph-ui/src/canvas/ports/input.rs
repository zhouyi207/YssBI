//! Inline controls are created for visible scalar ports; all writes use the graph edit owner.
pub(in crate::canvas) mod commit;
mod field;
mod render;

use crate::canvas::{GraphCanvas, GraphCommand};
use field::{Field, InputError};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{Context, EntityId, Window, prelude::*};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use yss_data_contract::SemanticType;
use yss_graph_document::{NodeId, PortAddress};
use yss_graph_editor::{
    EditorGraphMutation,
    projection::{EditorNodeModel, EditorPortModel, EditorProjectionModel},
};

#[derive(Default)]
pub(in crate::canvas) struct Inputs {
    projection: Option<Arc<EditorProjectionModel>>,
    fields: BTreeMap<PortAddress, Field>,
    visible_nodes: Vec<NodeId>,
    language: &'static str,
    generation: u64,
}

impl Inputs {
    pub(in crate::canvas) fn focused_node(
        &self,
        window: &Window,
        cx: &gpui_kit::App,
    ) -> Option<NodeId> {
        self.fields
            .iter()
            .find_map(|(address, field)| field.focused(window, cx).then_some(address.node_id))
    }

    /// The measured width of a port's inline field, in world units.
    pub(in crate::canvas) fn width(&self, address: &PortAddress) -> Option<f32> {
        self.fields.get(address).map(|field| f32::from(field.width))
    }

    pub(in crate::canvas) fn remeasure(
        &mut self,
        text_system: &gpui_kit::WindowTextSystem,
        cx: &gpui_kit::App,
    ) {
        for field in self.fields.values_mut() {
            field.measure(text_system, cx);
        }
    }
}

pub(in crate::canvas) fn eligible(port: &EditorPortModel) -> Option<SemanticType> {
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
        let projection_changed = self
            .port_inputs
            .projection
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(previous, projection));
        let language = yss_ui::text::locale();
        if !projection_changed
            && self.port_inputs.language == language
            && self
                .port_inputs
                .visible_nodes
                .iter()
                .copied()
                .eq(nodes.iter().map(|node| node.node_id))
        {
            return;
        }
        if projection_changed {
            let details = self.port_details.as_deref();
            self.port_inputs.fields.retain(|address, field| {
                let Some(port) = details
                    .and_then(|details| details.port(address))
                    .filter(|port| eligible(port) == Some(field.kind))
                else {
                    if field.dirty
                        || field.pending.is_some()
                        || field.error.is_some()
                        || field.focused(window, cx)
                    {
                        field.error = Some(InputError::Conflict);
                        return true;
                    }
                    return false;
                };
                field.install(port, version, window, cx);
                true
            });
            self.port_inputs.projection = Some(projection.clone());
        }
        self.port_inputs.visible_nodes.clear();
        self.port_inputs
            .visible_nodes
            .extend(nodes.iter().map(|node| node.node_id));
        self.port_inputs.language = language;
        self.port_inputs.generation = self.port_inputs.generation.wrapping_add(1);
        let generation = self.port_inputs.generation;
        for port in nodes.iter().flat_map(|node| node.ports.iter()) {
            let Some(kind) = eligible(port).filter(|kind| *kind != SemanticType::Binary) else {
                continue;
            };
            if let Some(field) = self.port_inputs.fields.get_mut(&port.address) {
                field.last_visible = generation;
                if kind == SemanticType::Text
                    && field.input.read(cx).presentation().placeholder().as_ref()
                        != yss_ui::text::t("conversion.text")
                {
                    yss_ui::text::input_placeholder(&field.input, "conversion.text", window, cx);
                    field.measure(window.text_system(), cx);
                }
                continue;
            }
            let input = cx.new(|cx| {
                let input = InputState::new(window, cx).default_value(field::text(port, kind));
                if kind == SemanticType::Numeric {
                    input.validate(field::numeric_draft)
                } else {
                    input.placeholder(yss_ui::text::translate("conversion.text"))
                }
            });
            let address = port.address.clone();
            let subscription =
                cx.subscribe_in(&input, window, move |view, input, event, window, cx| {
                    view.port_input_event(&address, input.entity_id(), event, window, cx);
                });
            let mut field = Field {
                kind,
                element_id: format!("port-value-{}", port.address).into(),
                input,
                version,
                base: port.input.clone(),
                dirty: false,
                pending: None,
                skip_blur: false,
                width: gpui_kit::px(28.),
                error: None,
                last_visible: generation,
                measurement: None,
                _subscription: subscription,
            };
            field.measure(window.text_system(), cx);
            self.port_inputs.fields.insert(port.address.clone(), field);
        }
        const RETAINED_OFFSCREEN_INPUTS: usize = 128;
        let mut evictable = self
            .port_inputs
            .fields
            .iter()
            .filter_map(|(address, field)| {
                (field.last_visible != generation
                    && !field.dirty
                    && field.pending.is_none()
                    && field.error.is_none()
                    && !field.focused(window, cx))
                .then_some((field.last_visible, address.clone()))
            })
            .collect::<Vec<_>>();
        if evictable.len() > RETAINED_OFFSCREEN_INPUTS {
            evictable.sort_unstable();
            let count = evictable.len() - RETAINED_OFFSCREEN_INPUTS;
            for (_, address) in evictable.into_iter().take(count) {
                self.port_inputs.fields.remove(&address);
            }
        }
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
                field.dirty = true;
                field.pending = None;
                field.error = None;
                field.measure(window.text_system(), cx);
                if !was_dirty {
                    cx.emit(gpui_kit::component::dock::PanelEvent::LayoutChanged);
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
                .port_details
                .as_ref()
                .and_then(|details| details.port(address))
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
            .port_details
            .as_ref()
            .and_then(|details| details.port(address))
        else {
            return;
        };
        if let Some(field) = self.port_inputs.fields.get_mut(address) {
            field.restore(port, self.graph.editing.version, window, cx);
            field.skip_blur = true;
        }
        window.focus(&self.focus, cx);
        cx.emit(gpui_kit::component::dock::PanelEvent::LayoutChanged);
        cx.notify();
    }
}
