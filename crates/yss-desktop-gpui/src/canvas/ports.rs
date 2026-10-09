//! Port presentation shares its type palette with connections; decisions stay backend-owned.
mod inspection;
mod menu;

use super::{GraphCanvas, presentation::State};
use crate::appearance;
use gpui::{Context, IntoElement, MouseButton, div, prelude::*, px, rgb};
use yss_data_contract::{SemanticType, ValueType};
use yss_graph_editor::projection::{ConnectionDecision, EditorPortModel, EditorPortTypeState};
use yss_node_protocol::PortDirection;

pub(crate) fn scalar_input_type(port: &EditorPortModel) -> Option<SemanticType> {
    if port.direction != PortDirection::Input || port.orphan {
        return None;
    }
    match &port.type_state {
        EditorPortTypeState::Exact {
            data_type:
                Some(ValueType::Scalar(
                    kind @ (SemanticType::Numeric | SemanticType::Binary | SemanticType::Text),
                )),
            ..
        } => Some(*kind),
        _ => None,
    }
}

pub(super) fn type_color(port: &EditorPortModel) -> u32 {
    let EditorPortTypeState::Exact {
        data_type: Some(value),
        ..
    } = &port.type_state
    else {
        return 0xd4d4d4;
    };
    let mut value = value;
    while let ValueType::Array(inner) | ValueType::DataSeries(inner) = value {
        value = inner;
    }
    match value {
        ValueType::Scalar(SemanticType::Numeric) => 0x5eead4,
        ValueType::Scalar(SemanticType::Binary) => 0xfb7185,
        ValueType::Scalar(SemanticType::Text) => 0xfbbf24,
        ValueType::Scalar(SemanticType::Datetime) => 0xc084fc,
        ValueType::DataFrame => 0x60a5fa,
        _ => 0xd4d4d4,
    }
}

impl GraphCanvas {
    pub(super) fn render_port(
        &self,
        port: &EditorPortModel,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let start = port.address.clone();
        let end = start.clone();
        let menu_address = start.clone();
        let output = port.direction == PortDirection::Output;
        let label = port
            .display
            .instance_label
            .as_deref()
            .unwrap_or(&port.display.label)
            .to_owned();
        let hovered = start.clone();
        let drag = self.connection_drag();
        let decision = drag.and_then(|drag| drag.decision(&port.address));
        let active = drag.is_some_and(|drag| drag.source == port.address);
        let target = drag.is_some_and(|drag| drag.target.as_ref() == Some(&port.address));
        let dimmed = !active && matches!(decision, Some(ConnectionDecision::Invalid { .. }));
        let color = if port.orphan {
            appearance::RED
        } else {
            type_color(port)
        };
        let state = self
            .presentation
            .ports
            .get(&port.address)
            .copied()
            .unwrap_or_default();
        let ring = if active {
            Some(appearance::BLUE)
        } else {
            match decision {
                Some(ConnectionDecision::Append) => Some(appearance::GREEN),
                Some(ConnectionDecision::Replace { .. }) => Some(appearance::AMBER),
                Some(ConnectionDecision::Invalid { .. }) if target => Some(appearance::RED),
                _ => match state {
                    State::Error => Some(appearance::RED),
                    State::Stale | State::Running => Some(appearance::AMBER),
                    State::Valid => Some(color),
                    State::Unexecuted | State::Partial => None,
                },
            }
        };
        let diameter = if compact { 20. } else { 10. };
        let dot = || {
            div()
                .size(px(diameter * self.zoom))
                .flex_shrink_0()
                .rounded_full()
                .bg(rgb(color))
                .border_1()
                .border_color(rgb(ring.unwrap_or(appearance::CANVAS)))
                .when(ring.is_some(), |dot| dot.border_2())
                .when(state == State::Stale, |dot| dot.border_dashed())
                .opacity(if dimmed && !target { 0.3 } else { 1. })
        };
        div()
            .id(gpui::SharedString::from(format!(
                "port-{}-{:?}",
                port.address.node_id, port.address.port
            )))
            .flex()
            .items_center()
            .gap(px(6. * self.zoom))
            .w_full()
            .min_w_0()
            .text_size(px(12. * self.zoom))
            .text_color(rgb(appearance::MUTED))
            .when(self.located_port.as_ref() == Some(&port.address), |view| {
                view.track_focus(&self.port_focus)
                    .bg(rgb(appearance::BLUE).opacity(0.2))
            })
            .cursor_crosshair()
            .on_hover(cx.listener(move |view, over, _, cx| {
                if let Some(drag) = view.connection_drag_mut() {
                    let target = if *over {
                        Some(hovered.clone())
                    } else if drag.target.as_ref() == Some(&hovered) {
                        None
                    } else {
                        return;
                    };
                    if drag.set_target(target) {
                        cx.notify();
                    }
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event, window, cx| {
                    view.begin_port(start.clone(), event, window, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                    view.show_port_menu(menu_address.clone(), event.position, window, cx)
                }),
            )
            .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, _, _, cx| view.end_port(end.clone(), cx)),
            )
            .when(!output, |view| {
                view.ml(px(-diameter / 2. * self.zoom))
                    .child(dot())
                    .when(!compact, |view| {
                        view.child(div().min_w_0().flex_1().truncate().child(label.clone()))
                    })
            })
            .when(output, |view| {
                view.mr(px(-diameter / 2. * self.zoom))
                    .justify_end()
                    .when(!compact, |view| {
                        view.child(div().min_w_0().truncate().child(label))
                    })
                    .child(dot())
            })
    }
}
