use super::SettingsPanel;
use crate::{preferences, text::t};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{InputEvent, InputState, NumberInput},
    menu::{DropdownMenu, PopupMenuItem},
    setting::{SettingField, SettingItem},
    switch::Switch,
};
use gpui_kit::{
    App, AppContext, Axis, Context, Entity, Focusable, IntoElement, SharedString, Subscription,
    WeakEntity, Window, div, prelude::*,
};
use std::{
    rc::Rc,
    sync::{Arc, LazyLock},
};
use yss_settings::UserSettings;

type Change<T> = Rc<dyn Fn(T, &mut App)>;

fn defaults() -> &'static UserSettings {
    static DEFAULTS: LazyLock<UserSettings> = LazyLock::new(UserSettings::default);
    &DEFAULTS
}

impl SettingsPanel {
    pub(super) fn preference_values<'a>(&'a self, cx: &'a App) -> &'a UserSettings {
        self.preference_draft
            .as_ref()
            .unwrap_or_else(|| preferences::current(cx))
    }

    fn preference<T, E>(
        &self,
        key: &'static str,
        description: &'static str,
        get: fn(&UserSettings) -> T,
        set: fn(&mut UserSettings, T),
        control: impl Fn(T, Change<T>, bool, &mut Window, &mut App) -> E + 'static,
        cx: &Context<Self>,
    ) -> SettingItem
    where
        T: Clone + PartialEq + Send + 'static,
        E: IntoElement + 'static,
    {
        let owner = cx.weak_entity();
        let default = get(defaults());
        let change: Change<T> = Rc::new({
            let owner = owner.clone();
            move |value, cx| {
                if let Err(error) = owner.update(cx, |view, cx| {
                    view.save_preferences(move |settings| set(settings, value), cx)
                }) {
                    tracing::debug!(%error, "Settings window closed");
                }
            }
        });
        SettingItem::new(
            t(key),
            SettingField::render(move |options, window, cx| {
                let value = owner
                    .upgrade()
                    .map(|owner| get(owner.read(cx).preference_values(cx)))
                    .unwrap_or_else(|| get(preferences::current(cx)));
                let disabled = options.is_disabled();
                let unchanged = value == default;
                let reset = default.clone();
                let reset_change = change.clone();
                div()
                    .id(key)
                    .w_64()
                    .max_w_full()
                    .when(options.layout() == Axis::Vertical, |field| field.w_full())
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().flex().justify_end().child(control(
                        value,
                        change.clone(),
                        disabled,
                        window,
                        cx,
                    )))
                    .child(
                        Button::new("reset-preference")
                            .small()
                            .ghost()
                            .icon(IconName::Undo2)
                            .accessibility_label(t("preferences.reset"))
                            .tooltip(t("preferences.reset"))
                            .disabled(disabled || unchanged)
                            .on_click(move |_, _, cx| reset_change(reset.clone(), cx)),
                    )
            }),
        )
        .description(SharedString::from(t(description)))
        .keywords([key])
        .disabled(self.loading || self.task.is_some() || self.knowledge.pending)
    }

    pub(super) fn preference_switch(
        &self,
        key: &'static str,
        description: &'static str,
        get: fn(&UserSettings) -> bool,
        set: fn(&mut UserSettings, bool),
        cx: &Context<Self>,
    ) -> SettingItem {
        self.preference(
            key,
            description,
            get,
            set,
            move |value, change, disabled, _, _| {
                Switch::new(key)
                    .checked(value)
                    .disabled(disabled)
                    .on_click(move |value, _, cx| change(*value, cx))
            },
            cx,
        )
    }

    pub(super) fn preference_choice<T: Clone + PartialEq + Send + 'static>(
        &self,
        key: &'static str,
        description: &'static str,
        get: fn(&UserSettings) -> T,
        set: fn(&mut UserSettings, T),
        choices: Vec<(T, SharedString)>,
        cx: &Context<Self>,
    ) -> SettingItem {
        let choices = Arc::new(choices);
        self.preference(
            key,
            description,
            get,
            set,
            move |value, change, disabled, _, _| {
                let label = choices
                    .iter()
                    .find(|(candidate, _)| *candidate == value)
                    .map(|(_, label)| label.clone())
                    .unwrap_or_else(|| t("preferences.default").into());
                let choices = choices.clone();
                Button::new(key)
                    .w_full()
                    .min_w_0()
                    .label(label)
                    .disabled(disabled)
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, _, _| {
                        choices
                            .iter()
                            .fold(menu, |menu, (candidate, label)| {
                                let candidate = candidate.clone();
                                let change = change.clone();
                                menu.item(
                                    PopupMenuItem::new(label.clone())
                                        .checked(candidate == value)
                                        .on_click(move |_, _, cx| change(candidate.clone(), cx)),
                                )
                            })
                            .scrollable(true)
                    })
            },
            cx,
        )
    }

    pub(super) fn preference_font(
        &self,
        key: &'static str,
        description: &'static str,
        get: fn(&UserSettings) -> Option<String>,
        set: fn(&mut UserSettings, Option<String>),
        cx: &Context<Self>,
    ) -> SettingItem {
        let fonts = self.font_names.clone();
        self.preference(
            key,
            description,
            get,
            set,
            move |value, change, disabled, _, _| {
                let label = value
                    .as_ref()
                    .map(|name| SharedString::from(name.clone()))
                    .unwrap_or_else(|| t("preferences.defaultFont").into());
                let fonts = fonts.clone();
                Button::new(key)
                    .w_full()
                    .min_w_0()
                    .label(label)
                    .disabled(disabled)
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, _, _| {
                        let reset = change.clone();
                        let menu = menu.item(
                            PopupMenuItem::new(t("preferences.defaultFont"))
                                .checked(value.is_none())
                                .on_click(move |_, _, cx| reset(None, cx)),
                        );
                        fonts
                            .iter()
                            .fold(menu, |menu, name| {
                                let selected = value.as_deref() == Some(name.as_ref());
                                let name = name.clone();
                                let change = change.clone();
                                menu.item(
                                    PopupMenuItem::new(name.clone()).checked(selected).on_click(
                                        move |_, _, cx| change(Some(name.to_string()), cx),
                                    ),
                                )
                            })
                            .scrollable(true)
                    })
            },
            cx,
        )
    }

    pub(super) fn preference_number(
        &self,
        key: &'static str,
        description: &'static str,
        get: fn(&UserSettings) -> f64,
        set: fn(&mut UserSettings, f64),
        bounds: (f64, f64, f64),
        cx: &Context<Self>,
    ) -> SettingItem {
        let owner = cx.weak_entity();
        self.preference(
            key,
            description,
            get,
            set,
            move |value, change, disabled, window, cx| {
                let state = window.use_keyed_state(key, cx, |window, cx| {
                    NumberPreference::new(value, get, change, bounds, owner.clone(), window, cx)
                });
                NumberInput::new(&state.read(cx).input)
                    .w_32()
                    .disabled(disabled)
            },
            cx,
        )
    }
}

struct NumberPreference {
    input: Entity<InputState>,
    saved: f64,
    _subscriptions: Vec<Subscription>,
}
impl NumberPreference {
    fn new(
        value: f64,
        get: fn(&UserSettings) -> f64,
        change: Change<f64>,
        (min, max, step): (f64, f64, f64),
        owner: WeakEntity<SettingsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value.to_string())
                .min(min)
                .max(max)
                .step(step)
        });
        let subscription = cx.subscribe_in(&input, window, move |_, input, event, _, cx| {
            if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                && let Ok(value) = input.read(cx).value().parse::<f64>()
                && value.is_finite()
            {
                change(value.clamp(min, max), cx);
            }
        });
        let observe = owner.upgrade().map(|owner| {
            cx.observe_in(&owner, window, move |state, owner, window, cx| {
                let value = get(owner.read(cx).preference_values(cx));
                if state.saved != value {
                    let input = state.input.read(cx);
                    let editing = input.focus_handle(cx).is_focused(window)
                        && input.value().parse::<f64>().ok() != Some(state.saved);
                    state.saved = value;
                    if !editing {
                        state.input.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx)
                        });
                    }
                    cx.notify();
                }
            })
        });
        Self {
            input,
            saved: value,
            _subscriptions: std::iter::once(subscription).chain(observe).collect(),
        }
    }
}
