//! Shortcut rows retain drafts by command/context, independently of settings search ordering.
use super::SettingsPanel;
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    setting::{SettingGroup, SettingItem},
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Render, SharedString, Subscription, WeakEntity,
    Window, prelude::*,
};
use std::rc::Rc;
use yss_settings::KeyBindingOverride;
use yss_ui::lifecycle::LogReleased;
use yss_ui::{
    keymap, preferences,
    text::{self, t},
};

/// Areas in display order, keyed by the first identifier of a command's context.
const AREAS: [(&str, &str); 9] = [
    ("Workbench", "preferences.shortcuts.areas.general"),
    ("GraphCanvas", "preferences.shortcuts.areas.graph"),
    ("MindCanvas", "preferences.shortcuts.areas.mind"),
    ("DocumentEditor", "preferences.shortcuts.areas.document"),
    ("DatabaseEditor", "preferences.shortcuts.areas.database"),
    ("ChartEditor", "preferences.shortcuts.areas.chart"),
    (
        "AssistantConversation",
        "preferences.shortcuts.areas.assistant",
    ),
    ("Settings", "preferences.shortcuts.areas.settings"),
    ("", "preferences.shortcuts.areas.other"),
];

fn area(context: &str) -> usize {
    let root = context
        .split(|character: char| !character.is_alphanumeric())
        .next()
        .unwrap_or_default();
    let root = root.strip_suffix("Breadcrumb").unwrap_or(root);
    AREAS
        .iter()
        .position(|(name, _)| *name == root)
        .unwrap_or(AREAS.len() - 1)
}

impl SettingsPanel {
    pub(super) fn keybindings(&self, cx: &mut Context<Self>) -> Vec<SettingGroup> {
        let owner = cx.weak_entity();
        let mut items: [Vec<SettingItem>; AREAS.len()] = Default::default();
        for command in keymap::commands(cx) {
            let id = command.id().to_owned();
            let context = command.context().to_owned();
            let defaults = command.defaults().to_vec();
            let identity = SharedString::from(format!("shortcut:{id}:{context}"));
            let owner = owner.clone();
            let label: SharedString = command.label().into();
            let keywords = vec![
                label.clone(),
                id.clone().into(),
                context.clone().into(),
                defaults.join(" ").into(),
            ];
            let Some(area_items) = items.get_mut(area(&context)) else {
                continue;
            };
            area_items.push(
                super::fields::resettable_item(label, None, move |_, window, cx| {
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
                    let overridden = owner.upgrade().is_some_and(|owner| {
                        owner
                            .read(cx)
                            .preference_values(cx)
                            .keybindings
                            .iter()
                            .any(|binding| {
                                binding.action == id
                                    && binding.context.as_deref() == Some(context.as_str())
                            })
                    });
                    let reset = overridden.then(|| {
                        let editor = editor.downgrade();
                        Rc::new(move |cx: &mut App| {
                            editor
                                .update(cx, |editor, cx| editor.change(None, cx))
                                .log_released();
                        }) as super::fields::Reset
                    });
                    (reset, editor.into_any_element())
                })
                .keywords(keywords)
                .disabled(self.loading || self.task.is_some() || self.knowledge.pending),
            );
        }
        AREAS
            .iter()
            .zip(items)
            .filter(|(_, items)| !items.is_empty())
            .map(|((_, title), items)| SettingGroup::new().title(t(title)).items(items))
            .collect()
    }
}

struct ShortcutEditor {
    action: String,
    context: String,
    defaults: Vec<String>,
    owner: WeakEntity<SettingsPanel>,
    input: Entity<InputState>,
    saved: Option<String>,
    notifications: yss_ui::notifications::Notifications,
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
        let value = Self::effective(saved.as_deref(), &defaults).to_owned();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder(text::translate("preferences.shortcuts.unbound"))
        });
        let subscription = cx.subscribe_in(&input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.commit(cx);
            }
        });
        let preference_subscription =
            cx.observe_global_in::<preferences::Preferences>(window, |this, window, cx| {
                let saved = Self::saved(&this.action, &this.context, cx);
                if this.saved != saved {
                    this.saved = saved;
                    let value = Self::effective(this.saved.as_deref(), &this.defaults).to_owned();
                    this.input
                        .update(cx, |input, cx| input.set_value(value, window, cx));
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
            notifications: yss_ui::notifications::Notifications::new(window),
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

    fn effective<'a>(saved: Option<&'a str>, defaults: &'a [String]) -> &'a str {
        saved
            .or_else(|| defaults.last().map(String::as_str))
            .unwrap_or_default()
    }

    /// An empty shortcut is saved as an override, which disables the command's defaults.
    fn commit(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().trim().to_owned();
        if text == Self::effective(self.saved.as_deref(), &self.defaults) {
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
            self.notifications.error(error.to_string(), cx);
            cx.notify();
            return;
        }
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
            self.notifications
                .error(text::translate("preferences.shortcuts.closed"), cx);
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
        Input::new(&self.input).w_64().max_w_full().disabled(busy)
    }
}
