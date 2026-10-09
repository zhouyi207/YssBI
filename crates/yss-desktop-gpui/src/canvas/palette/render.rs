use super::*;
use crate::text::{activity_text, translate};
use gpui::{
    AnyElement, IntoElement, MouseButton, Render, SharedString, div, prelude::*, px, uniform_list,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::Input,
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;

impl Render for NodePalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if (self.refresh_pending || self.language != crate::text::locale())
            && !self.creating
            && self.current(cx)
        {
            self.query(cx);
        }
        crate::text::input_placeholder(
            &self.search,
            "canvas.nodePalette.searchPlaceholder",
            window,
            cx,
        );
        div()
            .id("node-palette")
            .track_focus(&self.focus)
            .size_full()
            .p_2()
            .rounded_md()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_2()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(Self::key_down))
            .child(if let Some(form) = &self.configuration {
                form.clone().into_any_element()
            } else {
                self.render_browser(cx)
            })
    }
}

impl NodePalette {
    fn render_browser(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.can_create(cx);
        let toggle_label = if self.browser.collapsed.is_empty() {
            "canvas.nodePalette.collapseAll"
        } else {
            "canvas.nodePalette.expandAll"
        };
        div()
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(Input::new(&self.search).small().w_full())
                    .child(
                        Button::new("palette-toggle-all")
                            .ghost()
                            .small()
                            .icon(if self.browser.collapsed.is_empty() {
                                IconName::ChevronsDownUp
                            } else {
                                IconName::ChevronsUpDown
                            })
                            .tooltip(translate(toggle_label))
                            .disabled(
                                self.browser.searching()
                                    || self.browser.visible.is_empty()
                                    || self.creating,
                            )
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.browser.toggle_all();
                                view.scroll_to_active();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                Checkbox::new("configure-before-creating")
                    .label(translate("canvas.nodePalette.configureFirst"))
                    .checked(self.configure_first)
                    .disabled(self.creating)
                    .on_click(cx.listener(|view, value: &bool, _, cx| {
                        view.configure_first = *value;
                        cx.notify();
                    })),
            )
            .when(self.loading, |body| {
                body.child(div().text_xs().child(translate("common.loading")))
            })
            .when(!self.current(cx), |body| {
                body.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(translate("native.workbench.creationChanged")),
                )
            })
            .when_some(self.error, |body, error| {
                body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(translate(error)),
                        )
                        .when(error == "native.canvas.catalogFailed", |body| {
                            body.child(
                                Button::new("palette-retry")
                                    .ghost()
                                    .small()
                                    .label(translate("common.retry"))
                                    .disabled(self.loading || !self.current(cx))
                                    .on_click(cx.listener(|view, _, _, cx| view.query(cx))),
                            )
                        }),
                )
            })
            .when(
                self.browser.visible.is_empty() && !self.loading && self.error.is_none(),
                |body| {
                    body.child(
                        div()
                            .p_3()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(translate("canvas.nodePalette.noMatches")),
                    )
                },
            )
            .child(
                uniform_list(
                    "node-palette-items",
                    self.browser.visible.len(),
                    cx.processor(move |view, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| view.render_row(index, enabled, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .min_h_0()
                .track_scroll(&self.scroll),
            )
            .into_any_element()
    }

    fn render_row(
        &self,
        index: usize,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let row = self.browser.row(index)?;
        let generation = self.browser.generation;
        let body = div()
            .id(SharedString::from(row.id.clone()))
            .h(px(32.))
            .w_full()
            .pl(px(6. + row.depth as f32 * 12.))
            .pr_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded_sm()
            .text_sm();
        Some(match &row.content {
            ActivityRowContent::Category { label, .. } => body
                .child(
                    Icon::new(
                        if !self.browser.searching() && self.browser.collapsed.contains(&row.id) {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        },
                    )
                    .size_3(),
                )
                .child(div().min_w_0().truncate().child(activity_text(label)))
                .when(!self.browser.searching() && !self.creating, |body| {
                    body.cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.browser.generation == generation {
                                view.browser.toggle(index);
                                view.scroll_to_active();
                                cx.notify();
                            }
                        }))
                })
                .into_any_element(),
            ActivityRowContent::Item(ActivityItem::Node {
                title,
                available,
                creation,
                ..
            }) => {
                let node_type = match creation {
                    NodeCreation::Static { node_type_id }
                    | NodeCreation::ParameterizedStatic { node_type_id, .. }
                    | NodeCreation::ResourceBound { node_type_id, .. } => node_type_id,
                };
                let hint = format!("{title}\n{node_type}");
                body.tooltip(move |window, cx| Tooltip::new(hint.clone()).build(window, cx))
                    .child(
                        Icon::new(if matches!(creation, NodeCreation::ResourceBound { .. }) {
                            IconName::Braces
                        } else {
                            IconName::Frame
                        })
                        .size_3(),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(title.clone()))
                    .when(!available, |body| {
                        body.text_color(cx.theme().muted_foreground).child(
                            div()
                                .text_xs()
                                .child(translate("canvas.nodePalette.unavailable")),
                        )
                    })
                    .when(*available && self.browser.active == Some(index), |body| {
                        body.bg(cx.theme().accent)
                    })
                    .when(*available && enabled, |body| {
                        body.cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.browser.generation == generation {
                                    view.choose(index, window, cx);
                                }
                            }))
                    })
                    .into_any_element()
            }
            _ => return None,
        })
    }
}
