//! Next-turn options stay with the draft; model restrictions come from the catalog.
use super::ConversationPanel;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, div, prelude::*, px};
use yss_harness_contract::{HarnessMode, ReasoningEffort};

impl ConversationPanel {
    pub(super) fn effort_choices(&self) -> (Vec<ReasoningEffort>, Option<ReasoningEffort>) {
        let Some(selection) = self.selection() else {
            return (vec![], None);
        };
        let Some((provider, model)) = self.model_config(&selection) else {
            return (vec![], None);
        };
        let mut choices = model.reasoning_efforts.clone();
        if choices.is_empty() {
            choices.extend([
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
            ]);
        }
        let default = provider.reasoning_defaults.get(&model.id).copied();
        if let Some(default) = default
            && !choices.contains(&default)
        {
            choices.push(default);
        }
        (choices, default)
    }

    pub(super) fn reconcile_effort(&mut self) {
        if self.catalog.is_some()
            && self
                .options
                .reasoning_effort
                .is_some_and(|effort| !self.effort_choices().0.contains(&effort))
        {
            self.options.reasoning_effort = None;
        }
    }

    fn set_effort(&mut self, effort: Option<ReasoningEffort>, cx: &mut Context<Self>) {
        let (choices, default) = self.effort_choices();
        if effort.is_some_and(|effort| !choices.contains(&effort)) {
            return;
        }
        self.options.reasoning_effort = effort.filter(|effort| Some(*effort) != default);
        cx.notify();
    }

    pub(super) fn mode_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let selected = self.options.mode;
        Button::new("assistant-mode")
            .xsmall()
            .ghost()
            .dropdown_caret(true)
            .label(crate::text::t(mode_key(selected)))
            .accessibility_label(crate::text::t("panel.assistantMode"))
            .tooltip(crate::text::t(mode_hint(selected)))
            .dropdown_menu(move |mut menu, _, _| {
                for mode in [HarnessMode::Write, HarnessMode::Ask] {
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::element(move |_, cx| {
                            div()
                                .max_w(px(280.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_xs().child(crate::text::t(mode_key(mode))))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(crate::text::t(mode_hint(mode))),
                                )
                        })
                        .checked(selected == mode)
                        .on_click(move |_, _, cx| {
                            let _ = owner.update(cx, |view, cx| view.set_mode(mode, cx));
                        }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    pub(super) fn effort_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let (choices, default) = self.effort_choices();
        let explicit = self.options.reasoning_effort;
        let selected = explicit.or(default);
        let label = selected.map(effort_label).unwrap_or_else(|| {
            crate::text::t(if choices.is_empty() {
                "panel.assistantReasoningEffort"
            } else {
                "panel.assistantEffortUnknown"
            })
        });
        let owner = cx.entity().downgrade();
        Button::new("assistant-effort")
            .xsmall()
            .ghost()
            .max_w(px(144.))
            .icon(IconName::Lightbulb)
            .label(label)
            .dropdown_caret(true)
            .accessibility_label(crate::text::t("panel.assistantReasoningEffort"))
            .tooltip(crate::text::t(if choices.is_empty() {
                "panel.assistantEffortUnavailable"
            } else {
                "settings.models.nextTurn"
            }))
            .disabled(choices.is_empty() && explicit.is_none())
            .dropdown_menu(move |mut menu, _, _| {
                let reset = owner.clone();
                menu = menu.item(
                    PopupMenuItem::new(crate::text::t("panel.assistantResetEffort"))
                        .disabled(explicit.is_none())
                        .on_click(move |_, _, cx| {
                            let _ = reset.update(cx, |view, cx| view.set_effort(None, cx));
                        }),
                );
                if !choices.is_empty() {
                    menu = menu.separator();
                }
                for effort in &choices {
                    let effort = *effort;
                    let owner = owner.clone();
                    let label = if Some(effort) == default {
                        crate::text::format(
                            "panel.assistantEffortDefault",
                            &[("value", effort_label(effort).into_owned())],
                        )
                    } else {
                        effort_label(effort).into_owned()
                    };
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(selected == Some(effort))
                            .on_click(move |_, _, cx| {
                                let _ =
                                    owner.update(cx, |view, cx| view.set_effort(Some(effort), cx));
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }
}

fn mode_key(mode: HarnessMode) -> &'static str {
    match mode {
        HarnessMode::Ask => "panel.assistantModes.ask",
        HarnessMode::Write => "panel.assistantModes.write",
    }
}
fn mode_hint(mode: HarnessMode) -> &'static str {
    match mode {
        HarnessMode::Ask => "panel.assistantModeHint.ask",
        HarnessMode::Write => "panel.assistantModeHint.write",
    }
}
fn effort_label(effort: ReasoningEffort) -> std::borrow::Cow<'static, str> {
    crate::text::t(match effort {
        ReasoningEffort::Low => "panel.assistantEffort.low",
        ReasoningEffort::Medium => "panel.assistantEffort.medium",
        ReasoningEffort::High => "panel.assistantEffort.high",
    })
}
