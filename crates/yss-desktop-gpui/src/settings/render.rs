use super::{BackInSettings, Page, SaveSettings, SettingsPanel, models::provider_name};
use crate::text::t;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable,
    breadcrumb::{Breadcrumb, BreadcrumbItem},
    button::Button,
    group_box::GroupBoxVariant,
    menu::{DropdownMenu, PopupMenuItem},
    setting::{SettingGroup, SettingItem, SettingPage, Settings},
};
use gpui_kit::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*};
use yss_harness_contract::{LanguageModelAuthentication, LanguageModelSelection};

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_key_placeholder(window, cx);
        let owner = cx.weak_entity();
        let mut groups = vec![
            SettingGroup::new().item(
                SettingItem::render(move |_, _, cx| {
                    owner
                        .update(cx, |view, cx| view.header(cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .keywords([t("settings.sections.ai"), t("settings.models.providers")]),
            ),
        ];
        match self.page {
            Page::Overview => {
                groups.push(self.overview(cx));
                groups.push(self.assistant_preferences(cx));
            }
            Page::Providers => groups.push(self.providers(cx)),
            Page::Provider => groups.extend(self.provider_form(cx)),
        }
        let owner = cx.weak_entity();
        let knowledge_status = SettingGroup::new().item(
            SettingItem::render(move |_, _, cx| {
                owner
                    .update(cx, |view, cx| view.knowledge_status(cx))
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .keywords([
                t("settings.knowledge.title"),
                t("settings.knowledge.document"),
                t("settings.knowledge.description"),
                t("settings.knowledge.rebuild"),
            ]),
        );
        let knowledge = [
            category_header("settings.knowledge.title"),
            knowledge_status,
        ]
        .into_iter()
        .chain(self.knowledge_page(cx));
        let appearance = self.appearance_preferences(cx);
        let documents = self.document_preferences(cx);
        let data = self.data_preferences(cx);
        let workspace = self.workspace_preferences(cx);
        let logs = self.log_preferences(cx);
        let keybindings = self.keybindings(cx);
        let hidden_header = div().hidden().style().clone();
        div()
            .id("native-settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &SaveSettings, window, cx| view.save(window, cx)))
            .child(
                div().flex_1().min_h_0().min_w_0().child(
                    // Settings retains its translated search placeholder for the keyed lifetime.
                    Settings::new((
                        gpui_kit::ElementId::from("application-settings"),
                        gpui_kit::SharedString::from(crate::text::locale()),
                    ))
                    .default_selected_index(self.initial_page)
                    .with_group_variant(GroupBoxVariant::Normal)
                    .pages([
                        SettingPage::new(t("settings.sections.ai"))
                            .icon(IconName::Bot)
                            .header_style(&hidden_header)
                            .groups(groups),
                        SettingPage::new(t("settings.knowledge.title"))
                            .icon(IconName::Search)
                            .header_style(&hidden_header)
                            .groups(knowledge),
                        SettingPage::new(t("settings.sections.appearance"))
                            .icon(IconName::Languages)
                            .header_style(&hidden_header)
                            .group(category_header("settings.sections.appearance"))
                            .group(appearance),
                        SettingPage::new(t("preferences.sections.documents"))
                            .header_style(&hidden_header)
                            .group(category_header("preferences.sections.documents"))
                            .group(documents),
                        SettingPage::new(t("preferences.sections.data"))
                            .header_style(&hidden_header)
                            .group(category_header("preferences.sections.data"))
                            .group(data),
                        SettingPage::new(t("preferences.sections.workspace"))
                            .header_style(&hidden_header)
                            .group(category_header("preferences.sections.workspace"))
                            .group(workspace),
                        SettingPage::new(t("preferences.sections.logs"))
                            .header_style(&hidden_header)
                            .group(category_header("preferences.sections.logs"))
                            .group(logs),
                        SettingPage::new(t("preferences.sections.keybindings"))
                            .header_style(&hidden_header)
                            .group(category_header("preferences.sections.keybindings"))
                            .group(keybindings),
                    ]),
                ),
            )
    }
}

impl SettingsPanel {
    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut breadcrumb = Breadcrumb::new().flex_1().min_w_0();
        let mut overview = BreadcrumbItem::new(t("settings.sections.ai"));
        if self.page != Page::Overview {
            overview =
                overview
                    .disabled(self.busy())
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.navigate(Page::Overview, window, cx);
                    }));
        }
        breadcrumb = breadcrumb.child(overview);
        if self.page != Page::Overview {
            let mut providers = BreadcrumbItem::new(t("settings.models.providers"));
            if self.page == Page::Provider {
                providers =
                    providers
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.navigate(Page::Providers, window, cx);
                        }));
            }
            breadcrumb = breadcrumb.child(providers);
        }
        if self.page == Page::Provider {
            let title = self
                .editor
                .as_ref()
                .map(|draft| draft.display_name(cx))
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| t("native.settings.newProvider").into());
            breadcrumb = breadcrumb.child(BreadcrumbItem::new(title).min_w_0().truncate());
        }
        breadcrumb_row()
            .id("settings-breadcrumb")
            .focusable()
            .focus_visible(|style| style.bg(cx.theme().secondary))
            .tab_stop(true)
            .key_context("SettingsBreadcrumb")
            .on_action(cx.listener(|view, _: &BackInSettings, window, cx| {
                if !view.busy() {
                    match view.page {
                        Page::Provider => view.navigate(Page::Providers, window, cx),
                        Page::Providers => view.navigate(Page::Overview, window, cx),
                        Page::Overview => {}
                    }
                }
            }))
            .child(breadcrumb)
            .when(self.page == Page::Providers, |header| {
                header.child(self.add_provider_button(cx))
            })
            .into_any_element()
    }

    fn add_provider_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("add-provider")
            .icon(IconName::Plus)
            .label(t("settings.models.addProvider"))
            .disabled(
                self.busy()
                    || self
                        .catalog
                        .as_ref()
                        .is_none_or(|catalog| catalog.presets.is_empty()),
            )
            .on_click(cx.listener(|view, _, window, cx| {
                let preset = view
                    .catalog
                    .as_ref()
                    .and_then(|catalog| catalog.presets.first())
                    .cloned();
                if let Some(preset) = preset {
                    view.edit_provider(None, Some(preset), window, cx);
                }
            }))
    }

    fn overview(&self, cx: &mut Context<Self>) -> SettingGroup {
        SettingGroup::new()
            .item(
                self.render_field(
                    t("settings.models.defaultModel"),
                    t("native.settings.defaultModelHint"),
                    |view, cx| view.default_model_button(cx),
                    cx,
                )
                .keywords([t("settings.sections.ai")]),
            )
            .item(
                self.render_field(
                    t("settings.models.modelProviders"),
                    t("settings.models.modelProvidersDescription"),
                    |view, cx| {
                        Button::new("configure-providers")
                            .label(t("settings.models.configure"))
                            .icon(IconName::ChevronRight)
                            .disabled(view.busy())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.navigate(Page::Providers, window, cx)
                            }))
                    },
                    cx,
                )
                .keywords([t("settings.sections.ai")]),
            )
    }

    fn default_model_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut label = t("native.settings.noDefaultModel").into_owned();
        let mut options = vec![];
        if let Some(catalog) = &self.catalog {
            for provider in &catalog.providers {
                for model in &provider.config.models {
                    let selection = LanguageModelSelection {
                        provider_id: provider.config.id.clone(),
                        model_id: model.id.clone(),
                    };
                    let title = format!("{} / {}", provider_name(&provider.config), model.name);
                    if catalog.default_model.as_ref() == Some(&selection) {
                        label = title.clone();
                    }
                    let enabled = provider.has_api_key
                        || provider.config.authentication == LanguageModelAuthentication::None;
                    options.push((selection, title, enabled));
                }
            }
        }
        let generation = self.generation;
        let owner = cx.weak_entity();
        Button::new("settings-default-model")
            .label(label)
            .disabled(self.busy() || options.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                for (selection, title, enabled) in &options {
                    let owner = owner.clone();
                    let selection = selection.clone();
                    menu = menu.item(
                        PopupMenuItem::new(title.clone())
                            .disabled(!enabled)
                            .on_click(move |_, window, cx| {
                                if let Err(error) = owner.update(cx, |view, cx| {
                                    if view.generation == generation {
                                        view.set_default(selection.clone(), window, cx);
                                    }
                                }) {
                                    tracing::debug!(%error, "Settings view closed");
                                }
                            }),
                    );
                }
                menu
            })
    }

    fn providers(&self, cx: &mut Context<Self>) -> SettingGroup {
        let mut group = SettingGroup::new();
        let Some(catalog) = &self.catalog else {
            return group;
        };
        if catalog.providers.is_empty() {
            group = group.item(
                SettingItem::render(|_, _, _| t("native.settings.providersEmpty"))
                    .keywords([t("settings.sections.ai"), t("settings.models.providers")]),
            );
        }
        for provider in &catalog.providers {
            let id = provider.config.id.clone();
            let info = crate::text::format(
                "native.settings.providerSummary",
                &[
                    ("value0", provider.config.models.len().to_string()),
                    (
                        "value1",
                        t(
                            if provider.config.authentication == LanguageModelAuthentication::None {
                                "settings.models.noAuthentication"
                            } else if provider.has_api_key {
                                "native.settings.credentialsSaved"
                            } else {
                                "native.settings.credentialsRequired"
                            },
                        )
                        .to_string(),
                    ),
                ],
            );
            group = group.item(
                self.render_field(
                    provider_name(&provider.config),
                    info,
                    move |view, cx| {
                        let id = id.clone();
                        Button::new(gpui_kit::SharedString::from(format!("provider-edit-{id}")))
                            .label(t("detail.constantValue.edit"))
                            .disabled(view.busy())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                let provider = view
                                    .catalog
                                    .as_ref()
                                    .and_then(|catalog| {
                                        catalog
                                            .providers
                                            .iter()
                                            .find(|provider| provider.config.id == id)
                                    })
                                    .cloned();
                                if let Some(provider) = provider {
                                    view.edit_provider(Some(provider), None, window, cx);
                                }
                            }))
                    },
                    cx,
                )
                .keywords([t("settings.sections.ai"), t("settings.models.providers")]),
            );
        }
        group
    }
}

fn category_header(key: &'static str) -> SettingGroup {
    SettingGroup::new().item(
        SettingItem::render(move |_, _, _| {
            breadcrumb_row().child(Breadcrumb::new().child(BreadcrumbItem::new(t(key))))
        })
        .keywords([t(key)]),
    )
}

fn breadcrumb_row() -> gpui_kit::Div {
    div().w_full().min_w_0().h_8().flex().items_center().gap_3()
}
