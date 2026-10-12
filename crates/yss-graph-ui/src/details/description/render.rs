use super::{Content, NodeDescription};
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{Context, IntoElement, Render, Window, div, prelude::*, rems};
use yss_ui::inspector::{self, InlineMessage, InspectorSection, Pager};
use yss_ui::text::translate;

const PAGE_COLUMNS: usize = 50;

impl Render for NodeDescription {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh(window, cx);
        if self.source.is_none() {
            return div().into_any_element();
        }
        InspectorSection::new("description-result", translate("detail.description.result"))
            .when_some(
                match &self.content {
                    Content::Ready(columns) => Some(columns.len()),
                    _ => None,
                },
                InspectorSection::count,
            )
            .collapsible(
                self.expanded,
                cx.listener(|view, open: &bool, window, cx| {
                    view.expanded = *open;
                    if view.expanded && matches!(view.content, Content::Unloaded) {
                        view.read(window, cx);
                    }
                    cx.notify();
                }),
            )
            .when(self.expanded, |section| section.child(self.render_body(cx)))
            .into_any_element()
    }
}

impl NodeDescription {
    fn render_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        match &self.content {
            Content::Ready(columns) => {
                let pages = columns.len().div_ceil(PAGE_COLUMNS).max(1);
                let start = self.page * PAGE_COLUMNS;
                let end = (start + PAGE_COLUMNS).min(columns.len());
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .id("description-columns")
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .when(pages > 1, |list| {
                                list.max_h(rems(34.))
                                    .overflow_y_scroll()
                                    .track_scroll(&self.columns_scroll)
                            })
                            .children(columns.get(start..end).unwrap_or_default().iter().cloned()),
                    )
                    .when(pages > 1, |body| {
                        body.child(
                            div().flex().justify_end().child(
                                Pager::new("description-pager", self.page, pages)
                                    .label(format!("{}–{} / {}", start + 1, end, columns.len()))
                                    .on_change(cx.listener(|view, page: &usize, _, cx| {
                                        view.page = *page;
                                        view.columns_scroll.set_offset(gpui_kit::Point::default());
                                        cx.notify();
                                    })),
                            ),
                        )
                    })
                    .into_any_element()
            }
            Content::Failed => div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(InlineMessage::error(translate(
                    "native.results.unavailable",
                )))
                .child(
                    Button::new("description-retry")
                        .small()
                        .ghost()
                        .label(translate("common.retry"))
                        .on_click(cx.listener(|view, _, window, cx| view.read(window, cx))),
                )
                .into_any_element(),
            state => inspector::hint(
                translate(match state {
                    Content::Loading | Content::Unloaded => "common.loading",
                    Content::Invalid => "detail.description.invalid",
                    _ => "detail.description.unavailable",
                }),
                cx,
            )
            .into_any_element(),
        }
    }
}
