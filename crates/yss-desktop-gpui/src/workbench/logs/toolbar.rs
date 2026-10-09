//! Native search, domain/level menus and live-view controls.
use super::*;
use gpui::Div;
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};

impl LogsPanel {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let owner = cx.weak_entity();
        let domain = self.domain;
        let levels = self.levels.clone();
        let level_owner = owner.clone();
        let follow_key = if self.auto_scroll {
            "log.autoScrollEnabled"
        } else {
            "log.autoScrollDisabled"
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("refresh-logs")
                    .small()
                    .ghost()
                    .icon(IconName::Replace)
                    .tooltip(crate::text::translate("log.refresh"))
                    .accessibility_label(crate::text::translate("log.refresh"))
                    .disabled(self.connecting)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.recovery_attempts = 0;
                        view.truncated = false;
                        if view.auto_scroll {
                            view.scroll.scroll_to_bottom();
                        }
                        view.connect(window, cx);
                    })),
            )
            .child(
                Button::new("follow-logs")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowDown)
                    .selected(self.auto_scroll)
                    .tooltip(crate::text::translate(follow_key))
                    .accessibility_label(crate::text::translate(follow_key))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.auto_scroll = !view.auto_scroll;
                        if view.auto_scroll {
                            view.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("log-domain")
                    .small()
                    .ghost()
                    .dropdown_caret(true)
                    .label(crate::text::translate(entry::domain_key(domain)))
                    .dropdown_menu(move |mut menu, _, _| {
                        for choice in [
                            None,
                            Some(LogDomain::Application),
                            Some(LogDomain::Execution),
                            Some(LogDomain::System),
                            Some(LogDomain::Graph),
                            Some(LogDomain::Data),
                            Some(LogDomain::Ui),
                        ] {
                            let owner = owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(crate::text::translate(entry::domain_key(
                                    choice,
                                )))
                                .checked(domain == choice)
                                .on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.domain != choice {
                                            view.domain = choice;
                                            view.refilter();
                                            cx.notify();
                                        }
                                    });
                                }),
                            );
                        }
                        menu
                    }),
            )
            .child(
                Button::new("log-levels")
                    .small()
                    .ghost()
                    .icon(IconName::ListFilter)
                    .selected(self.levels.len() != LEVELS.len())
                    .tooltip(crate::text::translate("log.level"))
                    .accessibility_label(crate::text::translate("log.level"))
                    .dropdown_menu(move |mut menu, _, _| {
                        for level in LEVELS {
                            let owner = level_owner.clone();
                            menu = menu.item(
                                PopupMenuItem::new(entry::level_label(level))
                                    .checked(levels.contains(&level))
                                    .on_click(move |_, _, cx| {
                                        let _ = owner.update(cx, |view, cx| {
                                            if view.levels.contains(&level) {
                                                view.levels.retain(|value| *value != level);
                                            } else {
                                                view.levels.push(level);
                                            }
                                            view.refilter();
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .child(
                div().flex_1().min_w(px(120.)).max_w(px(320.)).child(
                    Input::new(&self.search)
                        .small()
                        .prefix(Icon::new(IconName::Search).size_3()),
                ),
            )
            .child(
                Button::new("clear-logs-view")
                    .small()
                    .ghost()
                    .icon(IconName::Trash)
                    .tooltip(crate::text::translate("native.workbench.clearDisplay"))
                    .accessibility_label(crate::text::translate("native.workbench.clearDisplay"))
                    .disabled(self.connecting)
                    .on_click(cx.listener(|view, _, _, cx| view.clear_display(cx))),
            )
    }
}
