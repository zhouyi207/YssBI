use super::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::{IntoElement, Render, div, prelude::*};
use yss_node_catalog::PortCountPolicy;
use yss_ui::inspector::{self, InspectorHeader, InspectorSection, PropertyField};
use yss_ui::text::translate;

impl Render for NodeCreationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_language(window, cx);
        let busy = !self.can_edit(cx);
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(InspectorHeader::new(
                IconName::Workflow,
                self.target.title.clone(),
            ))
            .child(
                div()
                    .id("node-creation-fields")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(self.form.is_none() && self.preparing.is_some(), |body| {
                        body.child(inspector::status_band(
                            inspector::hint(translate("common.loading"), cx),
                            cx,
                        ))
                    })
                    .when(!self.ports.is_empty(), |body| {
                        body.child(self.render_counts(busy, cx))
                    })
                    .when(self.form.is_some(), |body| {
                        body.child(self.parameters.clone())
                    }),
            )
            .child(self.render_footer(busy, cx))
    }
}

impl NodeCreationView {
    fn render_counts(&self, busy: bool, cx: &App) -> InspectorSection {
        InspectorSection::new("port-counts", translate("canvas.nodePalette.pinCounts")).child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(self.ports.iter().map(|port| {
                    let control = match &port.input {
                        Some(input) => Input::new(input)
                            .small()
                            .w_20()
                            .disabled(busy)
                            .into_any_element(),
                        None => match port.policy {
                            PortCountPolicy::Fixed => div().text_sm().child("1"),
                            _ => div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(translate("canvas.nodePalette.derivedPins")),
                        }
                        .into_any_element(),
                    };
                    PropertyField::new(port.title.clone())
                        .inline()
                        .control(control)
                })),
        )
    }

    /// Commits and outcomes stay visible below the scrolling form.
    fn render_footer(&self, busy: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_shrink_0()
            .px_3()
            .py_2()
            .flex()
            .flex_col()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
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
                    .when(self.form.is_none() && self.preparing.is_none(), |row| {
                        row.child(
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
