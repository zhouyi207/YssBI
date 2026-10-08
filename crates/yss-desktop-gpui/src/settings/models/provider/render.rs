use super::protocol_label;
use crate::{settings::SettingsPanel, text::translate as t};
use gpui::{AnyElement, Context, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable,
    button::Button,
    combobox::Combobox,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit_assets::IconName;
use yss_harness_contract::{
    LanguageModelAuthentication as Authentication, LanguageModelProtocol as Protocol,
};

impl SettingsPanel {
    pub(in crate::settings::models) fn connection_fields(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = self.editor.as_ref().unwrap();
        let busy = self.busy();
        let mut content = div()
            .flex()
            .flex_col()
            .child(self.render_field(
                &t("settings.models.customName"),
                &t("settings.models.customNameDescription"),
                Input::new(&draft.custom_name).disabled(busy),
                cx,
            ))
            .child(
                self.render_field(
                    &t("settings.models.providerName"),
                    "",
                    Combobox::new(&draft.preset)
                        .w_full()
                        .placeholder(draft.name.clone())
                        .icon(IconName::ChevronDown)
                        .check_icon(IconName::Check)
                        .search_placeholder(t("settings.models.searchProviders"))
                        .disabled(
                            busy || self
                                .catalog
                                .as_ref()
                                .is_none_or(|catalog| catalog.presets.is_empty()),
                        )
                        .empty(|_, cx| {
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(t("settings.models.noMatchingProviders"))
                        }),
                    cx,
                ),
            )
            .child(self.protocol_field(cx))
            .child(self.render_field(
                &t("settings.models.baseUrl"),
                &t("settings.models.endpointHint"),
                Input::new(&draft.base_url).disabled(busy),
                cx,
            ))
            .child(self.authentication_field(cx));
        if draft.authentication == Authentication::ApiKey {
            content = content.child(self.render_field(
                "API Key",
                &t(if self.replacement_key_required() {
                    "settings.models.newProviderKeyHint"
                } else {
                    "settings.models.keyHint"
                }),
                Input::new(&draft.key).disabled(busy),
                cx,
            ));
        }
        content.into_any_element()
    }

    fn protocol_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let draft = self.editor.as_ref().unwrap();
        let selected = draft.protocol;
        let epoch = self.epoch;
        let owner = cx.weak_entity();
        self.render_field(
            &t("settings.models.protocol"),
            "",
            Button::new("provider-protocol")
                .label(protocol_label(selected))
                .disabled(self.busy())
                .dropdown_menu(move |mut menu, _, _| {
                    for protocol in [
                        Protocol::OpenAiResponses,
                        Protocol::OpenAiChat,
                        Protocol::Anthropic,
                        Protocol::Gemini,
                    ] {
                        let owner = owner.clone();
                        menu = menu.item(
                            PopupMenuItem::new(protocol_label(protocol))
                                .checked(selected == protocol)
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.epoch == epoch {
                                            view.change_protocol(protocol, window, cx);
                                        }
                                    });
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )
    }

    fn authentication_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let draft = self.editor.as_ref().unwrap();
        let selected = draft.authentication;
        let allow_none = matches!(
            draft.protocol,
            Protocol::OpenAiResponses | Protocol::OpenAiChat
        );
        let epoch = self.epoch;
        let owner = cx.weak_entity();
        self.render_field(
            &t("settings.models.authentication"),
            "",
            Button::new("provider-auth")
                .label(if selected == Authentication::None {
                    t("settings.models.noAuthentication")
                } else {
                    "API Key".into()
                })
                .disabled(self.busy())
                .dropdown_menu(move |mut menu, _, _| {
                    for (label, value) in [
                        ("API Key".to_owned(), Authentication::ApiKey),
                        (t("settings.models.noAuthentication"), Authentication::None),
                    ] {
                        let owner = owner.clone();
                        menu = menu.item(
                            PopupMenuItem::new(label)
                                .checked(value == selected)
                                .disabled(value == Authentication::None && !allow_none)
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if view.epoch == epoch {
                                            view.change_authentication(value, window, cx);
                                        }
                                    });
                                }),
                        );
                    }
                    menu
                }),
            cx,
        )
    }
}
