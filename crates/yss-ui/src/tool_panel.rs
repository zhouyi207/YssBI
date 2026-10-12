use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    AnyElement, App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*,
};

use crate::appearance::{HEADER_BUTTON_SIZE, HEADER_HEIGHT};

#[derive(IntoElement)]
pub struct ToolPanel {
    title: SharedString,
    tools: Vec<AnyElement>,
    content: AnyElement,
}

impl ToolPanel {
    pub fn new(title: impl Into<SharedString>, content: impl IntoElement) -> Self {
        Self {
            title: title.into(),
            tools: Vec::new(),
            content: content.into_any_element(),
        }
    }

    pub fn tool(mut self, tool: impl IntoElement) -> Self {
        self.tools.push(tool.into_any_element());
        self
    }

    pub fn tools(mut self, tools: impl IntoIterator<Item = AnyElement>) -> Self {
        self.tools.extend(tools);
        self
    }
}

impl RenderOnce for ToolPanel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_xs()
            .child(
                div()
                    .h(HEADER_HEIGHT)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_shrink_0().child(self.title))
                    .when(!self.tools.is_empty(), |header| {
                        header.child(
                            div()
                                .id("tool-panel-tools")
                                .h_full()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_1()
                                .overflow_x_scroll()
                                .whitespace_nowrap()
                                .children(self.tools),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.content),
            )
    }
}

pub fn button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .xsmall()
        .h(HEADER_BUTTON_SIZE)
        .min_w(HEADER_BUTTON_SIZE)
        .ghost()
        .flex_shrink_0()
}
