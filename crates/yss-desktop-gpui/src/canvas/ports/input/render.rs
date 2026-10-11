use super::*;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Escape, Input},
    tooltip::Tooltip,
};
use gpui_kit::{AnyElement, IntoElement, MouseButton, div, px};

impl GraphCanvas {
    pub(in crate::canvas::ports) fn render_port_input(
        &self,
        port: &EditorPortModel,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let kind = eligible(port)?;
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
            // The regular Switch has fixed pixel geometry; a toggle button keeps
            // its keyboard/accessibility behavior while this canvas track scales.
            Button::new(gpui_kit::SharedString::from(format!(
                "port-value-{address}"
            )))
            .small()
            .ghost()
            .accessibility_label(label)
            .toggled(checked)
            .w(px(28. * self.zoom))
            .h(px(16. * self.zoom))
            .min_w_0()
            .min_h_0()
            .p_0()
            .rounded_full()
            .child(
                div()
                    .relative()
                    .w(px(28. * self.zoom))
                    .h(px(16. * self.zoom))
                    .rounded_full()
                    .bg(if checked {
                        cx.theme().primary
                    } else {
                        cx.theme().input
                    })
                    .child(
                        div()
                            .absolute()
                            .top(px(2. * self.zoom))
                            .left(px(if checked { 14. } else { 2. } * self.zoom))
                            .size(px(12. * self.zoom))
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
                .small()
                .aria_label(label)
                .w(field.width * self.zoom)
                // Input::h configures multiline content; the single-line frame uses Styled.
                .map(|input| gpui_kit::Styled::h(input, px(18. * self.zoom)))
                .min_h_0()
                .text_size(px(10. * self.zoom))
                .line_height(px(18. * self.zoom))
                .px(px(4. * self.zoom))
                .py_0()
                .rounded(px(2. * self.zoom))
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
            InputError::Conflict => Some(crate::text::translate("native.canvas.commandFailed")),
            InputError::Number => {
                crate::workbench::parse_number(&field.input.read(cx).value()).err()
            }
        });
        Some(
            div()
                .id(gpui_kit::SharedString::from(format!(
                    "port-input-{address}"
                )))
                .flex_shrink_0()
                .flex()
                .h(px(18. * self.zoom))
                .items_center()
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
