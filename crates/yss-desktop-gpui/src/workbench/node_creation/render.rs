use super::*;
use crate::text::translate;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::{IntoElement, Render, div, prelude::*};
use yss_node_catalog::PortCountPolicy;

impl Render for NodeCreationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_language(window, cx);
        let busy = !self.can_edit(cx);
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(self.target.title.clone()),
            )
            .child(
                div()
                    .id("node-creation-fields")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(self.form.is_none() && self.preparing.is_some(), |body| {
                        body.child(translate("common.loading"))
                    })
                    .when(!self.ports.is_empty(), |body| {
                        body.child(self.render_counts(busy, cx))
                    })
                    .when(self.form.is_some(), |body| {
                        body.child(self.parameters.clone())
                    }),
            )
            .when(!self.current(cx), |body| {
                body.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(translate("native.workbench.creationChanged")),
                )
            })
            .children(self.error.as_ref().map(|error| {
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone())
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("creation-back")
                            .small()
                            .ghost()
                            .label(translate("canvas.nodePalette.back"))
                            .disabled(self.creating)
                            .on_click(cx.listener(|view, _, _, cx| {
                                if !view.creating {
                                    cx.emit(CreationEvent::Back);
                                }
                            })),
                    )
                    .when(self.form.is_none() && self.preparing.is_none(), |body| {
                        body.child(
                            Button::new("creation-retry")
                                .small()
                                .ghost()
                                .label(translate("common.retry"))
                                .disabled(!self.current(cx))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.query(Preparation::default(), window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("creation-create")
                            .small()
                            .primary()
                            .label(translate("canvas.nodePalette.create"))
                            .disabled(busy || self.form.is_none())
                            .on_click(cx.listener(|view, _, window, cx| view.create(window, cx))),
                    ),
            )
    }
}

impl NodeCreationView {
    fn render_counts(&self, busy: bool, cx: &gpui_kit::App) -> gpui_kit::Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(translate("canvas.nodePalette.pinCounts")),
            )
            .children(self.ports.iter().map(|port| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_xs()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().min_w_0().child(port.title.clone()))
                            .when_some(port.input.as_ref(), |row, input| {
                                row.child(
                                    Input::new(input)
                                        .small()
                                        .w(gpui_kit::px(84.))
                                        .disabled(busy),
                                )
                            })
                            .when(port.input.is_none(), |row| {
                                let value = match port.policy {
                                    PortCountPolicy::Fixed => "1".to_owned(),
                                    _ => translate("canvas.nodePalette.derivedPins"),
                                };
                                row.child(
                                    div().text_color(cx.theme().muted_foreground).child(value),
                                )
                            }),
                    )
                    .children(
                        port.error
                            .as_ref()
                            .map(|error| div().text_color(cx.theme().danger).child(error.clone())),
                    )
            }))
    }
}
