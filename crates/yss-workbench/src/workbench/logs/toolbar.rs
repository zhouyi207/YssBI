//! Native search, domain/level menus and live-view controls.
use super::*;
use gpui_kit::AnyElement;
use gpui_kit::component::{
    Disableable, Selectable, Sizable,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use yss_ui::{appearance::HEADER_BUTTON_SIZE, tool_panel};

impl LogsPanel {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let owner = cx.weak_entity();
        let domain = self.domain;
        let levels = self.levels.clone();
        let level_owner = owner.clone();
        let follow_key = if self.auto_scroll {
            "log.autoScrollEnabled"
        } else {
            "log.autoScrollDisabled"
        };
        vec![
            tool_panel::button("refresh-logs")
                .icon(IconName::Replace)
                .tooltip(crate::text::translate("log.refresh"))
                .accessibility_label(crate::text::translate("log.refresh"))
                .disabled(self.connecting)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.recovery_attempts = 0;
                    if view.auto_scroll {
                        view.scroll.scroll_to_bottom();
                    }
                    view.connect(window, cx);
                }))
                .into_any_element(),
            tool_panel::button("follow-logs")
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
                }))
                .into_any_element(),
            tool_panel::button("log-domain")
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
                                if let Err(error) = owner.update(cx, |view, cx| {
                                    if view.domain != choice {
                                        view.domain = choice;
                                        view.refilter();
                                        cx.notify();
                                    }
                                }) {
                                    tracing::debug!(%error, "Log panel closed before domain selection");
                                }
                            }),
                        );
                    }
                    menu
                })
                .into_any_element(),
            tool_panel::button("log-levels")
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
                                    if let Err(error) = owner.update(cx, |view, cx| {
                                        if view.levels.contains(&level) {
                                            view.levels.retain(|value| *value != level);
                                        } else {
                                            view.levels.push(level);
                                        }
                                        view.refilter();
                                        cx.notify();
                                    }) {
                                        tracing::debug!(%error, "Log panel closed before level selection");
                                    }
                                }),
                        );
                    }
                    menu
                })
                .into_any_element(),
            div()
                .w_48()
                .flex_shrink_0()
                .child(
                    Input::new(&self.search)
                        .xsmall()
                        .min_h(HEADER_BUTTON_SIZE)
                        .max_h(HEADER_BUTTON_SIZE)
                        .prefix(Icon::new(IconName::Search).size_3()),
                )
                .into_any_element(),
            tool_panel::button("clear-logs-view")
                .icon(IconName::Trash)
                .tooltip(crate::text::translate("native.workbench.clearDisplay"))
                .accessibility_label(crate::text::translate("native.workbench.clearDisplay"))
                .disabled(self.connecting)
                .on_click(cx.listener(|view, _, _, cx| view.clear_display(cx)))
                .into_any_element(),
        ]
    }
}
