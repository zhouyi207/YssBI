use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    input::{Input, InputState},
};
use gpui_kit::{App, Div, Entity, FontWeight, SharedString, Stateful, div, prelude::*, rems};

pub fn header(title: impl Into<SharedString>) -> Div {
    div()
        .h(crate::appearance::HEADER_HEIGHT)
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .truncate()
                .child(title.into()),
        )
}

pub fn search(input: &Entity<InputState>) -> Div {
    div().flex_shrink_0().p_2().child(
        Input::new(input)
            .small()
            .prefix(Icon::new(IconName::Search).size_3()),
    )
}

pub fn list_item(item: Stateful<Div>, selected: bool, cx: &App) -> Stateful<Div> {
    item.h(rems(5.5))
        .w_full()
        .min_w_0()
        .px_3()
        .py_2()
        .flex()
        .flex_col()
        .gap_1()
        .aria_selected(selected)
        .bg(if selected {
            cx.theme().accent
        } else {
            cx.theme().background
        })
        .text_color(if selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        })
        .when(!selected, |item| {
            item.hover(|style| style.bg(cx.theme().muted))
        })
}

pub fn item_title(title: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .truncate()
        .child(title.into())
}

pub fn item_detail(detail: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .min_w_0()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .truncate()
        .child(detail.into())
}
