use super::protocol_label;
use crate::{settings::SettingsPanel, text::translate as t};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::Button,
    combobox::Combobox,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    setting::{SettingGroup, SettingItem},
};
use gpui_kit::{Context, IntoElement, div, prelude::*};
use yss_harness_contract::{
    LanguageModelAuthentication as Authentication, LanguageModelProtocol as Protocol,
};

impl SettingsPanel {
    pub(in crate::settings::models) fn connection_fields(
        &self,
        cx: &mut Context<Self>,
    ) -> SettingGroup {
        let mut group = SettingGroup::new()
            .item(self.render_field(
                t("settings.models.customName"),
                t("settings.models.customNameDescription"),
                |view, _| {
                    match &view.editor {
                        Some(draft) => Input::new(&draft.custom_name)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.render_field(
                t("settings.models.providerName"),
                "",
                |view, _| {
                    match &view.editor {
                        Some(draft) => Combobox::new(&draft.preset)
                            .w_full()
                            .placeholder(draft.name.clone())
                            .icon(IconName::ChevronDown)
                            .check_icon(IconName::Check)
                            .search_placeholder(t("settings.models.searchProviders"))
                            .disabled(
                                view.busy()
                                    || view
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
                            })
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.protocol_field(cx))
            .item(self.render_field(
                t("settings.models.baseUrl"),
                t("settings.models.endpointHint"),
                |view, _| {
                    match &view.editor {
                        Some(draft) => Input::new(&draft.base_url)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ))
            .item(self.authentication_field(cx));
        if self
            .editor
            .as_ref()
            .is_some_and(|draft| draft.authentication == Authentication::ApiKey)
        {
            group = group.item(self.render_field(
                "API Key",
                t(if self.replacement_key_required() {
                    "settings.models.newProviderKeyHint"
                } else {
                    "settings.models.keyHint"
                }),
                |view, _| {
                    match &view.editor {
                        Some(draft) => Input::new(&draft.key)
                            .disabled(view.busy())
                            .into_any_element(),
                        None => div().into_any_element(),
                    }
                },
                cx,
            ));
        }
        group
    }

    fn protocol_field(&self, cx: &mut Context<Self>) -> SettingItem {
        self.render_field(
            t("settings.models.protocol"),
            "",
            |view, cx| {
                let Some(draft) = &view.editor else {
                    return div().into_any_element();
                };
                let selected = draft.protocol;
                let epoch = view.epoch;
                let owner = cx.weak_entity();
                Button::new("provider-protocol")
                    .label(protocol_label(selected))
                    .disabled(view.busy())
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
                    })
                    .into_any_element()
            },
            cx,
        )
    }

    fn authentication_field(&self, cx: &mut Context<Self>) -> SettingItem {
        self.render_field(
            t("settings.models.authentication"),
            "",
            |view, cx| {
                let Some(draft) = &view.editor else {
                    return div().into_any_element();
                };
                let selected = draft.authentication;
                let allow_none = matches!(
                    draft.protocol,
                    Protocol::OpenAiResponses | Protocol::OpenAiChat
                );
                let epoch = view.epoch;
                let owner = cx.weak_entity();
                Button::new("provider-auth")
                    .label(if selected == Authentication::None {
                        t("settings.models.noAuthentication")
                    } else {
                        "API Key".into()
                    })
                    .disabled(view.busy())
                    .dropdown_menu(move |mut menu, _, _| {
                        for (label, value) in [
                            ("API Key".into(), Authentication::ApiKey),
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
                    })
                    .into_any_element()
            },
            cx,
        )
    }
}
