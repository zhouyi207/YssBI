use super::{Content, NodeDescription};
use crate::text::translate;
use gpui::{App, Context, IntoElement, Render, SharedString, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
};
use gpui_kit_assets::IconName;

const PAGE_COLUMNS: usize = 50;

pub(super) fn heading(
    id: &'static str,
    title: impl Into<SharedString>,
    open: bool,
    cx: &App,
) -> Button {
    let title = title.into();
    Button::new(id)
        .small()
        .ghost()
        .w_full()
        .h_7()
        .px_2()
        .rounded_none()
        .bg(cx.theme().muted.opacity(0.6))
        .accessibility_label(title.clone())
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_1p5()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(
                    Icon::new(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .size_3(),
                )
                .child(div().flex_1().min_w_0().truncate().child(title)),
        )
}

impl Render for NodeDescription {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh(window, cx);
        if self.source.is_none() {
            return div().into_any_element();
        }
        Collapsible::new()
            .w_full()
            .min_w_0()
            .open(self.expanded)
            .child(
                heading(
                    "description-result",
                    translate("detail.description.result"),
                    self.expanded,
                    cx,
                )
                .on_click(cx.listener(|view, _, window, cx| {
                    view.expanded = !view.expanded;
                    if view.expanded && matches!(view.content, Content::Unloaded) {
                        view.read(window, cx);
                    }
                    cx.notify();
                })),
            )
            .when(self.expanded, |section| {
                section.content(self.render_body(cx))
            })
            .into_any_element()
    }
}

impl NodeDescription {
    fn render_body(&self, cx: &mut Context<Self>) -> gpui::Div {
        let body = div().min_w_0().py_1p5().pl_2().text_xs();
        match &self.content {
            Content::Ready(columns) => {
                let start = self.page * PAGE_COLUMNS;
                let end = (start + PAGE_COLUMNS).min(columns.len());
                body.child(
                    div()
                        .id("description-columns")
                        .min_w_0()
                        .when(columns.len() > PAGE_COLUMNS, |list| {
                            list.max_h(gpui::px(480.))
                                .overflow_y_scroll()
                                .track_scroll(&self.columns_scroll)
                        })
                        .children(columns[start..end].iter().cloned()),
                )
                .when(columns.len() > PAGE_COLUMNS, |body| {
                    body.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_end()
                            .gap_1()
                            .child(format!("{}–{} / {}", start + 1, end, columns.len()))
                            .child(
                                Button::new("description-prev")
                                    .small()
                                    .ghost()
                                    .label(translate("sourceInspector.previous"))
                                    .disabled(self.page == 0)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.page = view.page.saturating_sub(1);
                                        view.columns_scroll.set_offset(gpui::Point::default());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("description-next")
                                    .small()
                                    .ghost()
                                    .label(translate("sourceInspector.next"))
                                    .disabled(end == columns.len())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        if let Content::Ready(columns) = &view.content
                                            && (view.page + 1) * PAGE_COLUMNS < columns.len()
                                        {
                                            view.page += 1;
                                            view.columns_scroll.set_offset(gpui::Point::default());
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                })
            }
            Content::Failed => body
                .child(
                    div()
                        .text_color(cx.theme().danger)
                        .child(translate("native.results.unavailable")),
                )
                .child(
                    Button::new("description-retry")
                        .small()
                        .ghost()
                        .label(translate("common.retry"))
                        .on_click(cx.listener(|view, _, window, cx| view.read(window, cx))),
                ),
            state => body
                .text_color(cx.theme().muted_foreground)
                .child(translate(match state {
                    Content::Loading | Content::Unloaded => "common.loading",
                    Content::Invalid => "detail.description.invalid",
                    _ => "detail.description.unavailable",
                })),
        }
    }
}
