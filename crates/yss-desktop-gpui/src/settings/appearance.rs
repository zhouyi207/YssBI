use super::SettingsPanel;
use crate::text::{self, t};
use gpui::{AnyElement, Context, IntoElement};
use gpui_component::{
    Disableable,
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
};

impl SettingsPanel {
    pub(super) fn appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        self.render_field(
            t("settings.labels.language"),
            t("settings.descriptions.language"),
            Button::new("settings-language")
                .label(language_name(text::locale()))
                .disabled(self.busy())
                .dropdown_menu(move |mut menu, _, _| {
                    for language in text::LANGUAGES {
                        let owner = owner.clone();
                        menu = menu.item(
                            PopupMenuItem::new(language_name(language))
                                .checked(text::locale() == language)
                                .on_click(move |_, _, cx| {
                                    let _ = owner
                                        .update(cx, |view, cx| view.change_language(language, cx));
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )
        .into_any_element()
    }

    fn change_language(&mut self, language: &'static str, cx: &mut Context<Self>) {
        if self.busy() || (text::locale() == language && self.preference_error.is_none()) {
            return;
        }
        self.task = Some(t("native.settings.savingPreferences"));
        self.preference_error = None;
        self.feedback = None;
        let store = self.services.preferences.clone();
        let job = self
            .services
            .executor
            .spawn_blocking(move || store.save_language(language));
        cx.spawn(async move |view, cx| {
            let result = job.await;
            let _ = view.update(cx, |view, cx| {
                view.task = None;
                if matches!(result, Ok(Ok(()))) {
                    text::set_locale(language);
                    cx.refresh_windows();
                } else {
                    view.preference_error = Some("native.settings.preferencesSaveFailed");
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn language_name(language: &str) -> &'static str {
    match language {
        "en-US" => "English",
        _ => "简体中文",
    }
}
