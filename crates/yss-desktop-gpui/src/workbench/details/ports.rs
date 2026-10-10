//! Input literals and instance operations share the graph's existing typed edit transaction.
mod list;
use super::{DetailsPanel, controls};
use crate::{appearance, canvas::GraphCommand};
use gpui::{AnyElement, Context, Entity, IntoElement, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputState},
};
use gpui_kit_assets::IconName;
use serde_json::Value;
use yss_data_contract::SemanticType;
use yss_graph_document::PortRef;
use yss_graph_editor::{
    EditorGraphMutation, PortPlacement,
    projection::{EditorEffectiveInputBinding, EditorPortModel, EditorPortTypeState},
};
use yss_node_protocol::PortDirection;

pub(super) struct PortField {
    pub model: EditorPortModel,
    pub(super) peers: Vec<super::connections::ConnectedPort>,
    pub(super) connections_page: usize,
    input: Option<(SemanticType, Entity<InputState>)>,
    error: Option<String>,
}

impl PortField {
    pub(super) fn accepts_projection(&self, next: &EditorPortModel) -> bool {
        if self.model.address != next.address {
            return false;
        }
        let mut current = self.model.clone();
        current.display = next.display.clone();
        match (&mut current.type_state, &next.type_state) {
            (
                EditorPortTypeState::Exact { display, .. },
                EditorPortTypeState::Exact { display: next, .. },
            )
            | (
                EditorPortTypeState::Constrained { display, .. },
                EditorPortTypeState::Constrained { display: next, .. },
            ) => {
                *display = next.clone();
            }
            _ => {}
        }
        current == *next
    }

    pub fn new(
        model: EditorPortModel,
        window: &mut Window,
        cx: &mut Context<DetailsPanel>,
    ) -> Self {
        let input = if let Some(kind) = crate::canvas::scalar_input_type(&model) {
            let value = model.input.as_ref().and_then(|input| {
                input
                    .literal_override
                    .as_ref()
                    .or(input.protocol_default.as_ref())
            });
            let text = value
                .map(|value| match value {
                    Value::String(value) => value.clone(),
                    _ => value.to_string(),
                })
                .unwrap_or_default();
            Some((
                kind,
                cx.new(|cx| InputState::new(window, cx).default_value(text)),
            ))
        } else {
            None
        };
        Self {
            model,
            peers: vec![],
            connections_page: 0,
            input,
            error: None,
        }
    }
}

impl DetailsPanel {
    fn apply_literal(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((kind, input)) = &self.ports[index].input else {
            return;
        };
        let text = input.read(cx).value();
        let value = match kind {
            SemanticType::Text => Ok(Value::String(text.to_string())),
            SemanticType::Numeric => controls::number(&text),
            SemanticType::Binary => Ok(Value::Bool(text == "true")),
            _ => return,
        };
        match value {
            Ok(value) => self.submit(
                GraphCommand::Edit(EditorGraphMutation::SetLiteral {
                    address: self.ports[index].model.address.clone(),
                    literal: Some(value),
                }),
                cx,
            ),
            Err(error) => self.ports[index].error = Some(error),
        }
        cx.notify();
    }

    pub(super) fn render_port_field(
        &self,
        index: usize,
        field: &PortField,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let port = &field.model;
        let epoch = self.epoch;
        let input = port.direction == PortDirection::Input;
        let address = port.address.clone();
        let disconnect = address.clone();
        let remove = address.clone();
        let mut content = div()
            .py_2()
            .flex()
            .flex_col()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(6.))
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(gpui::rgb(if input {
                                appearance::BLUE
                            } else {
                                appearance::GREEN
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div().text_sm().truncate().child(
                                    port.display
                                        .instance_label
                                        .as_deref()
                                        .unwrap_or(&port.display.label)
                                        .to_owned(),
                                ),
                            )
                            .child(controls::hint(port.accepted_type.to_string(), cx)),
                    )
                    .child(controls::hint(
                        if input {
                            crate::text::t("detail.nodeDoc.inputs")
                        } else {
                            crate::text::t("detail.nodeDoc.outputs")
                        },
                        cx,
                    )),
            );
        if port.orphan {
            content = content.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(crate::text::t("native.workbench.portSourceExpired")),
            );
        }
        if let Some((kind, state)) = &field.input {
            let disabled = busy || port.connections.current > 0;
            let editor = if *kind == SemanticType::Binary {
                Checkbox::new(("literal-toggle", index))
                    .label(crate::text::t("native.workbench.trueValue"))
                    .checked(state.read(cx).value() == "true")
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, value: &bool, window, cx| {
                        if !view.accepts_input(epoch, cx) {
                            return;
                        }
                        if let Some((_, input)) = &view.ports[index].input {
                            input.update(cx, |input, cx| {
                                input.set_value(value.to_string(), window, cx)
                            });
                        }
                        view.apply_literal(index, cx);
                    }))
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Input::new(state)
                            .small()
                            .flex_1()
                            .min_w_0()
                            .disabled(disabled),
                    )
                    .child(
                        controls::apply(("literal-apply", index), disabled).on_click(cx.listener(
                            move |view, _, _, cx| {
                                if view.accepts_input(epoch, cx) {
                                    view.apply_literal(index, cx)
                                }
                            },
                        )),
                    )
                    .into_any_element()
            };
            content = content.child(editor).child(controls::hint(
                match port.input.as_ref().map(|input| input.effective) {
                    Some(EditorEffectiveInputBinding::Connections) => {
                        crate::text::t("native.workbench.connectedValueHint")
                    }
                    Some(EditorEffectiveInputBinding::Literal) => {
                        crate::text::t("native.workbench.literalOverrideHint")
                    }
                    Some(EditorEffectiveInputBinding::ProtocolDefault) => {
                        crate::text::t("native.workbench.nodeDefaultHint")
                    }
                    _ => crate::text::t("native.workbench.unboundInputHint"),
                },
                cx,
            ));
        }
        content = content.child(self.render_port_connections(index, field, busy, cx));
        let mut actions = div().flex().items_center().gap_1();
        if port
            .input
            .as_ref()
            .is_some_and(|input| input.literal_override.is_some())
        {
            actions = actions.child(
                Button::new(("clear-literal", index))
                    .small()
                    .ghost()
                    .icon(IconName::Undo2)
                    .tooltip(crate::text::t("native.workbench.clearOverride"))
                    .disabled(busy || port.orphan)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.submit(
                                GraphCommand::Edit(EditorGraphMutation::SetLiteral {
                                    address: address.clone(),
                                    literal: None,
                                }),
                                cx,
                            )
                        }
                    })),
            );
        }
        if port.connections.current > 0 {
            actions = actions.child(
                Button::new(("disconnect-port", index))
                    .small()
                    .ghost()
                    .icon(IconName::Minus)
                    .label(crate::text::format(
                        "native.workbench.disconnectCount",
                        &[("value0", port.connections.current.to_string())],
                    ))
                    .disabled(busy)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.submit(
                                GraphCommand::Edit(EditorGraphMutation::DisconnectPort {
                                    address: disconnect.clone(),
                                }),
                                cx,
                            )
                        }
                    })),
            );
        }
        if port.can_remove {
            actions = actions.child(
                Button::new(("remove-port", index))
                    .small()
                    .ghost()
                    .icon(IconName::Close)
                    .tooltip(crate::text::t("native.workbench.removePort"))
                    .disabled(busy)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.submit(
                                GraphCommand::Edit(EditorGraphMutation::RemovePortInstance {
                                    address: remove.clone(),
                                }),
                                cx,
                            )
                        }
                    })),
            );
        }
        if port.can_remove
            && !port.orphan
            && let PortRef::Instance { template, .. } = &port.address.port
        {
            let siblings = self.ports.iter().filter(|field| matches!(&field.model.address.port, PortRef::Instance { template: key, .. } if key == template) && field.model.direction == port.direction).collect::<Vec<_>>();
            if let Some(position) = siblings
                .iter()
                .position(|field| field.model.address == port.address)
            {
                for (direction, neighbor) in [
                    (-1, position.checked_sub(1)),
                    (1, (position + 1 < siblings.len()).then_some(position + 1)),
                ] {
                    if let Some(neighbor) = neighbor
                        && let PortRef::Instance { instance_id, .. } =
                            siblings[neighbor].model.address.port
                    {
                        let address = port.address.clone();
                        let placement = if direction < 0 {
                            PortPlacement::Before(instance_id)
                        } else {
                            PortPlacement::After(instance_id)
                        };
                        actions = actions.child(
                            Button::new((
                                if direction < 0 {
                                    "port-up"
                                } else {
                                    "port-down"
                                },
                                index,
                            ))
                            .small()
                            .ghost()
                            .icon(if direction < 0 {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .tooltip(if direction < 0 {
                                crate::text::t("native.workbench.movePortUp")
                            } else {
                                crate::text::t("native.workbench.movePortDown")
                            })
                            .disabled(busy)
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    if view.accepts_input(epoch, cx) {
                                        view.submit(
                                            GraphCommand::Edit(
                                                EditorGraphMutation::MovePortInstance {
                                                    address: address.clone(),
                                                    placement: placement.clone(),
                                                },
                                            ),
                                            cx,
                                        )
                                    }
                                },
                            )),
                        );
                    }
                }
            }
        }
        content
            .child(actions)
            .children(field.error.as_ref().map(|error| {
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .into_any_element()
    }
}
