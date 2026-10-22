use super::*;
use crate::canvas::nodes::FIELD_HEIGHT;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Escape, Input},
    tooltip::Tooltip,
};
use gpui_kit::{AnyElement, Bounds, IntoElement, MouseButton, Pixels, div, px};

impl GraphCanvas {
    /// The editable control for an input's literal, placed over `lane` in canvas coordinates.
    pub(in crate::canvas) fn render_port_input(
        &self,
        port: &EditorPortModel,
        lane: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let kind = eligible(port)?;
        let zoom = self.zoom;
        let address = port.address.clone();
        let version = self.graph.editing.version;
        let label = port
            .display
            .instance_label
            .as_deref()
            .unwrap_or(&port.display.label)
            .to_owned();
        let field = self.port_inputs.fields.get(&address);
        let input = if kind == SemanticType::Binary {
            let checked = field::text(port, kind) == "true";
            let track = gpui_kit::size(lane.size.width, px(16. * zoom));
            let knob = px(12. * zoom);
            // The regular Switch has fixed pixel geometry; a toggle button keeps
            // its keyboard/accessibility behavior while this canvas track scales.
            Button::new(gpui_kit::SharedString::from(format!(
                "port-value-{address}"
            )))
            .small()
            .ghost()
            .accessibility_label(label)
            .toggled(checked)
            .w(track.width)
            .h(track.height)
            .min_w_0()
            .min_h_0()
            .p_0()
            .rounded_full()
            .child(
                div()
                    .relative()
                    .w(track.width)
                    .h(track.height)
                    .rounded_full()
                    .bg(if checked {
                        cx.theme().primary
                    } else {
                        cx.theme().input
                    })
                    .child(
                        div()
                            .absolute()
                            .top((track.height - knob) / 2.)
                            .left(if checked {
                                track.width - knob - px(2. * zoom)
                            } else {
                                px(2. * zoom)
                            })
                            .size(knob)
                            .rounded_full()
                            .bg(cx.theme().background),
                    ),
            )
            .disabled(!self.can_edit())
            .on_click(cx.listener(move |view, _, _, cx| {
                view.submit(
                    GraphCommand::Edit(EditorGraphMutation::SetLiteral {
                        address: address.clone(),
                        literal: Some(Value::Bool(!checked)),
                    }),
                    Some(version),
                    cx,
                );
            }))
            .into_any_element()
        } else {
            let field = field?;
            Input::new(&field.input)
                .id(field.element_id.clone())
                .small()
                .aria_label(label)
                .w(lane.size.width)
                // Input::h configures multiline content; the single-line frame uses Styled.
                .map(|input| gpui_kit::Styled::h(input, px(FIELD_HEIGHT * zoom)))
                .min_h_0()
                .text_size(px(11. * zoom))
                .line_height(px(FIELD_HEIGHT * zoom))
                .px(px(6. * zoom))
                .py_0()
                .rounded(px(FIELD_HEIGHT / 2. * zoom))
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .border_color(if field.error.is_some() {
                    cx.theme().danger
                } else {
                    cx.theme().input
                })
                .disabled(!self.can_edit())
                .into_any_element()
        };
        let address = port.address.clone();
        let error = field.and_then(|field| match field.error? {
            InputError::Conflict => Some(yss_ui::text::translate("native.canvas.commandFailed")),
            InputError::Number => yss_ui::input::number(&field.input.read(cx).value()).err(),
        });
        Some(
            div()
                .id(gpui_kit::SharedString::from(format!(
                    "port-input-{address}"
                )))
                .absolute()
                .left(lane.left())
                .top(lane.top())
                .w(lane.size.width)
                .h(lane.size.height)
                .flex()
                .items_center()
                .justify_end()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .on_action(cx.listener(move |view, _: &Escape, window, cx| {
                    view.cancel_port_input(&address, window, cx)
                }))
                .when_some(error, |view, error| {
                    view.tooltip(move |window, cx| Tooltip::new(error.clone()).build(window, cx))
                })
                .child(input)
                .into_any_element(),
        )
    }
}
