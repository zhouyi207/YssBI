//! Shortcut rows retain drafts by command/context, independently of settings search ordering.
use super::SettingsPanel;
use crate::{
    keymap, preferences,
    text::{self, t},
};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    setting::{SettingField, SettingGroup, SettingItem},
    v_flex,
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Render, SharedString, Subscription, WeakEntity,
    Window, div, prelude::*,
};
use yss_settings::KeyBindingOverride;

impl SettingsPanel {
    pub(super) fn keybindings(&self, cx: &mut Context<Self>) -> SettingGroup {
        let owner = cx.weak_entity();
        let mut group = SettingGroup::new();
        for command in keymap::commands(cx) {
            let id = command.id().to_owned();
            let context = command.context().to_owned();
            let defaults = command.defaults().to_vec();
            let current = preferences::current(cx).keybindings.iter().find(|binding| {
                binding.action == id && binding.context.as_deref() == Some(&context)
            });
            let effective = current
                .map(|binding| binding.keystroke.clone())
                .unwrap_or_else(|| defaults.join(" / "));
            let description = text::format(
                "preferences.shortcuts.scope",
                &[
                    ("action", id.clone()),
                    ("context", context.clone()),
                    (
                        "defaults",
                        if defaults.is_empty() {
                            text::translate("preferences.shortcuts.unbound")
                        } else {
                            defaults.join(" / ")
                        },
                    ),
                ],
            );
            let identity = SharedString::from(format!("shortcut:{id}:{context}"));
            let owner = owner.clone();
            let keywords = vec![id.clone(), context.clone(), effective, defaults.join(" ")];
            group = group.item(
                SettingItem::new(
                    command.label(),
                    SettingField::render(move |options, window, cx| {
                        let editor = window.use_keyed_state(identity.clone(), cx, |window, cx| {
                            ShortcutEditor::new(
                                id.clone(),
                                context.clone(),
                                defaults.clone(),
                                owner.clone(),
                                window,
                                cx,
                            )
                        });
                        div()
                            .w_64()
                            .max_w_full()
                            .when(options.layout() == gpui_kit::Axis::Vertical, |field| {
                                field.w_full()
                            })
                            .child(editor)
                    }),
                )
                .description(description)
                .keywords(keywords)
                .layout(gpui_kit::Axis::Vertical),
            );
        }
        group
    }
}

struct ShortcutEditor {
    action: String,
    context: String,
    defaults: Vec<String>,
    owner: WeakEntity<SettingsPanel>,
    input: Entity<InputState>,
    saved: Option<String>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl ShortcutEditor {
    fn new(
        action: String,
        context: String,
        defaults: Vec<String>,
        owner: WeakEntity<SettingsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved = Self::saved(&action, &context, cx);
        let value = saved
            .clone()
            .unwrap_or_else(|| defaults.last().cloned().unwrap_or_default());
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder(text::translate("preferences.shortcuts.placeholder"))
        });
        let subscription = cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            InputEvent::PressEnter { .. } | InputEvent::Blur => this.commit(cx),
            InputEvent::Change => {
                this.error = None;
                cx.notify();
            }
            _ => {}
        });
        let preference_subscription =
            cx.observe_global_in::<preferences::Preferences>(window, |this, window, cx| {
                let saved = Self::saved(&this.action, &this.context, cx);
                if this.saved != saved {
                    this.saved = saved;
                    this.error = None;
                    this.sync_input(window, cx);
                }
                cx.notify();
            });
        Self {
            action,
            context,
            defaults,
            owner,
            input,
            saved,
            error: None,
            _subscriptions: vec![subscription, preference_subscription],
        }
    }

    fn saved(action: &str, context: &str, cx: &App) -> Option<String> {
        preferences::current(cx)
            .keybindings
            .iter()
            .find(|binding| binding.action == action && binding.context.as_deref() == Some(context))
            .map(|binding| binding.keystroke.clone())
    }

    fn sync_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self
            .saved
            .clone()
            .unwrap_or_else(|| self.defaults.last().cloned().unwrap_or_default());
        self.input
            .update(cx, |input, cx| input.set_value(value, window, cx));
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().trim().to_owned();
        let effective = self
            .saved
            .as_deref()
            .or_else(|| self.defaults.last().map(String::as_str))
            .unwrap_or_default();
        if text == effective {
            return;
        }
        if text.is_empty() {
            self.error = Some(text::translate("preferences.shortcuts.empty"));
            cx.notify();
            return;
        }
        self.change(Some(text), cx);
    }

    fn change(&mut self, keystroke: Option<String>, cx: &mut Context<Self>) {
        let mut bindings = preferences::current(cx).keybindings.clone();
        bindings.retain(|binding| {
            binding.action != self.action || binding.context.as_deref() != Some(&self.context)
        });
        if let Some(keystroke) = &keystroke {
            bindings.push(KeyBindingOverride::new(
                self.action.clone(),
                Some(self.context.clone()),
                keystroke.clone(),
            ));
        }
        if let Err(error) = keymap::validate_bindings(&bindings, cx) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.error = None;
        let action = self.action.clone();
        let context = self.context.clone();
        if self
            .owner
            .update(cx, move |view, cx| {
                view.save_preferences(
                    move |settings| {
                        settings.keybindings.retain(|binding| {
                            binding.action != action || binding.context.as_deref() != Some(&context)
                        });
                        if let Some(keystroke) = keystroke {
                            settings.keybindings.push(KeyBindingOverride::new(
                                action,
                                Some(context),
                                keystroke,
                            ));
                        }
                    },
                    cx,
                );
            })
            .is_err()
        {
            self.error = Some(text::translate("preferences.shortcuts.closed"));
        }
        cx.notify();
    }
}

impl Render for ShortcutEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self
            .owner
            .upgrade()
            .is_none_or(|owner| owner.read(cx).busy());
        v_flex()
            .w_full()
            .min_w_0()
            .gap_1()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .flex_wrap()
                    .child(Input::new(&self.input).small().w_full().disabled(busy))
                    .child(
                        Button::new("remove")
                            .small()
                            .label(t("preferences.shortcuts.remove"))
                            .disabled(
                                busy || self.saved.as_deref() == Some("")
                                    || (self.saved.is_none() && self.defaults.is_empty()),
                            )
                            .on_click(
                                cx.listener(|this, _, _, cx| this.change(Some(String::new()), cx)),
                            ),
                    )
                    .child(
                        Button::new("reset")
                            .small()
                            .label(t("preferences.shortcuts.reset"))
                            .disabled(busy || self.saved.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.change(None, cx);
                                if this.error.is_none() {
                                    let value = this.defaults.last().cloned().unwrap_or_default();
                                    this.input
                                        .update(cx, |input, cx| input.set_value(value, window, cx));
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(if self.saved.as_deref() == Some("") {
                        "preferences.shortcuts.disabled"
                    } else {
                        "preferences.shortcuts.commitHint"
                    })),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
    }
}
