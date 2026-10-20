//! Input literals and instance operations share the graph's existing typed edit transaction.
mod list;
use super::GraphDetails;
use crate::canvas::GraphCommand;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    input::{Input, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
};
use gpui_kit::{
    Anchor, AnyElement, Context, Entity, IntoElement, Window, div, prelude::*, relative,
};
use serde_json::Value;
use yss_data_contract::SemanticType;
use yss_graph_document::PortRef;
use yss_graph_editor::{
    EditorGraphMutation, PortPlacement,
    projection::{EditorEffectiveInputBinding, EditorPortModel, EditorPortTypeState},
};
use yss_ui::appearance::HEADER_BUTTON_SIZE;
use yss_ui::inspector::{self, InlineMessage};
use yss_ui::text::{format, plural, translate};

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
        cx: &mut Context<GraphDetails>,
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

/// A secondary port command offered from the row's overflow menu.
struct PortAction {
    label: String,
    icon: IconName,
    mutation: EditorGraphMutation,
    disabled: bool,
}

impl GraphDetails {
    fn apply_literal(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((kind, input)) = &self.ports[index].input else {
            return;
        };
        let text = input.read(cx).value();
        let value = match kind {
            SemanticType::Text => Ok(Value::String(text.to_string())),
            SemanticType::Numeric => yss_ui::input::number(&text),
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
            Err(error) => {
                self.notifications.push(
                    gpui_kit::component::notification::Notification::error(error.clone())
                        .title(self.ports[index].model.display.label.to_string()),
                    cx,
                );
                self.ports[index].error = Some(error);
            }
        }
        cx.notify();
    }

    /// Identity row first; literal, peers and messages indent under the label, past the glyph.
    pub(super) fn render_port_field(
        &self,
        index: usize,
        field: &PortField,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let port = &field.model;
        let epoch = self.epoch;
        let label = port
            .display
            .instance_label
            .as_deref()
            .unwrap_or(&port.display.label)
            .to_owned();
        let literal_disabled = busy || port.connections.current > 0;
        let toggle = field
            .input
            .as_ref()
            .filter(|(kind, _)| *kind == SemanticType::Binary)
            .map(|(_, state)| state.read(cx).value() == "true");
        // The glyph paints in pixels; the theme font size is the rem that Root installs.
        let glyph = crate::canvas::port_glyph(port, cx.theme().font_size * 0.75, cx);
        let identity = div()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .child(glyph)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .child(label.clone()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(relative(0.4))
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(port.accepted_type.to_string()),
            )
            .when_some(toggle, |row, checked| {
                row.child(
                    Switch::new(("literal-toggle", index))
                        .small()
                        .checked(checked)
                        .disabled(literal_disabled)
                        .accessibility_label(label.clone())
                        .on_click(cx.listener(move |view, value: &bool, window, cx| {
                            if !view.accepts_input(epoch, cx) {
                                return;
                            }
                            if let Some((_, input)) =
                                view.ports.get(index).and_then(|field| field.input.clone())
                            {
                                input.update(cx, |input, cx| {
                                    input.set_value(value.to_string(), window, cx)
                                });
                            }
                            view.apply_literal(index, cx);
                        })),
                )
            })
            .child(self.render_port_connect(index, field, busy, cx))
            .child(
                self.render_port_menu(index, field, busy, cx)
                    .unwrap_or_else(|| {
                        div()
                            .w(HEADER_BUTTON_SIZE)
                            .flex_shrink_0()
                            .into_any_element()
                    }),
            );

        let mut details: Vec<AnyElement> = Vec::new();
        if port.orphan {
            details.push(
                InlineMessage::error(translate("native.workbench.portSourceExpired"))
                    .into_any_element(),
            );
        }
        if let Some((kind, state)) = &field.input {
            if *kind != SemanticType::Binary {
                details.push(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Input::new(state)
                                .small()
                                .flex_1()
                                .min_w_0()
                                .disabled(literal_disabled),
                        )
                        .child(
                            inspector::apply_button(("literal-apply", index), literal_disabled)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if view.accepts_input(epoch, cx) {
                                        view.apply_literal(index, cx)
                                    }
                                })),
                        )
                        .into_any_element(),
                );
            }
            details.push(
                inspector::hint(
                    match port.input.as_ref().map(|input| input.effective) {
                        Some(EditorEffectiveInputBinding::Connections) => {
                            translate("native.workbench.connectedValueHint")
                        }
                        Some(EditorEffectiveInputBinding::Literal) => {
                            translate("native.workbench.literalOverrideHint")
                        }
                        Some(EditorEffectiveInputBinding::ProtocolDefault) => {
                            translate("native.workbench.nodeDefaultHint")
                        }
                        _ => translate("native.workbench.unboundInputHint"),
                    },
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Some(peers) = self.render_port_connections(index, field, busy, cx) {
            details.push(peers);
        } else if field.input.is_none() {
            details.push(
                inspector::hint(translate("detail.nodeDoc.unconnected"), cx).into_any_element(),
            );
        }

        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(identity)
            .when(!details.is_empty(), |field| {
                field.child(
                    div()
                        .pl_5()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .children(details),
                )
            })
            .into_any_element()
    }

    fn render_port_menu(
        &self,
        index: usize,
        field: &PortField,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let port = &field.model;
        let address = &port.address;
        let mut actions = Vec::new();
        if port
            .input
            .as_ref()
            .is_some_and(|input| input.literal_override.is_some())
        {
            actions.push(PortAction {
                label: translate("native.workbench.clearOverride"),
                icon: IconName::Undo2,
                mutation: EditorGraphMutation::SetLiteral {
                    address: address.clone(),
                    literal: None,
                },
                disabled: busy || port.orphan,
            });
        }
        if port.connections.current > 0 {
            actions.push(PortAction {
                label: plural(
                    "native.workbench.disconnectCount",
                    usize::try_from(port.connections.current).unwrap_or(usize::MAX),
                ),
                icon: IconName::Unlink,
                mutation: EditorGraphMutation::DisconnectPort {
                    address: address.clone(),
                },
                disabled: busy,
            });
        }
        for (placement, up) in self.port_moves(port) {
            actions.push(PortAction {
                label: translate(if up {
                    "native.workbench.movePortUp"
                } else {
                    "native.workbench.movePortDown"
                }),
                icon: if up {
                    IconName::ArrowUp
                } else {
                    IconName::ArrowDown
                },
                mutation: EditorGraphMutation::MovePortInstance {
                    address: address.clone(),
                    placement,
                },
                disabled: busy,
            });
        }
        let removal = port.can_remove.then(|| PortAction {
            label: translate("native.workbench.removePort"),
            icon: IconName::Trash,
            mutation: EditorGraphMutation::RemovePortInstance {
                address: address.clone(),
            },
            disabled: busy,
        });
        if actions.is_empty() && removal.is_none() {
            return None;
        }
        let owner = cx.entity().downgrade();
        let epoch = self.epoch;
        let item = move |action: &PortAction| {
            let mutation = action.mutation.clone();
            PopupMenuItem::new(action.label.clone())
                .icon(action.icon)
                .disabled(action.disabled)
                .on_click(inspector::owner_command(
                    owner.clone(),
                    move |view: &mut GraphDetails, _, cx| {
                        if view.accepts_input(epoch, cx) {
                            view.submit(GraphCommand::Edit(mutation.clone()), cx);
                        }
                    },
                ))
        };
        Some(
            inspector::overflow_button(("port-actions", index))
                .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                    for action in &actions {
                        menu = menu.item(item(action));
                    }
                    if let Some(removal) = &removal {
                        if !actions.is_empty() {
                            menu = menu.separator();
                        }
                        menu = menu.item(item(removal));
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    /// Placements that move an instance port before or after its neighboring sibling.
    fn port_moves(&self, port: &EditorPortModel) -> Vec<(PortPlacement, bool)> {
        let PortRef::Instance { template, .. } = &port.address.port else {
            return Vec::new();
        };
        if !port.can_remove || port.orphan {
            return Vec::new();
        }
        let siblings = self
            .ports
            .iter()
            .filter(|field| {
                field.model.direction == port.direction
                    && matches!(&field.model.address.port, PortRef::Instance { template: key, .. } if key == template)
            })
            .collect::<Vec<_>>();
        let Some(position) = siblings
            .iter()
            .position(|field| field.model.address == port.address)
        else {
            return Vec::new();
        };
        [
            (position.checked_sub(1), true),
            (position.checked_add(1), false),
        ]
        .into_iter()
        .filter_map(|(neighbor, up)| {
            let PortRef::Instance { instance_id, .. } = siblings.get(neighbor?)?.model.address.port
            else {
                return None;
            };
            Some((
                if up {
                    PortPlacement::Before(instance_id)
                } else {
                    PortPlacement::After(instance_id)
                },
                up,
            ))
        })
        .collect()
    }
}
