use super::{Page, SaveSettings, SettingsPanel, models::provider_name};
use crate::text::t;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use yss_harness_contract::{LanguageModelAuthentication, LanguageModelSelection};

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_key_placeholder(window, cx);
        crate::text::input_placeholder(&self.search, "settings.searchPlaceholder", window, cx);
        let width = f32::from(window.viewport_size().width);
        self.render_width = width;
        let compact = width <= 720.;
        let padding = if compact {
            20.
        } else if width <= 900. {
            24.
        } else {
            32.
        };
        let empty = self.visible_categories(cx).is_empty();
        div()
            .id("native-settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .when(compact, |view| view.flex_col())
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &SaveSettings, window, cx| view.save(window, cx)))
            .child(self.navigation(compact, if width <= 900. { 184. } else { 208. }, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .when(!empty, |view| view.child(self.header(padding, cx)))
                    .child(self.status(cx))
                    .when(self.page == Page::Knowledge, |view| {
                        view.child(self.knowledge_status(cx))
                    })
                    .child(
                        div()
                            .id(("settings-content", self.epoch))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(920.))
                                    .mx_auto()
                                    .px(px(padding))
                                    .py_6()
                                    .child(if empty {
                                        div()
                                            .h(px(280.))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap_3()
                                            .child(Icon::new(IconName::Search).size_6())
                                            .child(div().text_sm().child(t("settings.noResults")))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(t("settings.noResultsHint")),
                                            )
                                            .child(
                                                Button::new("clear-settings-search")
                                                    .small()
                                                    .ghost()
                                                    .label(t("settings.clearSearch"))
                                                    .on_click(cx.listener(
                                                        |view, _, window, cx| {
                                                            view.search.update(cx, |search, cx| {
                                                                search.set_value("", window, cx)
                                                            })
                                                        },
                                                    )),
                                            )
                                            .into_any_element()
                                    } else {
                                        match self.page {
                                            Page::Overview => self.overview(cx),
                                            Page::Providers => self.providers(cx),
                                            Page::Provider => self.provider_form(cx),
                                            Page::Knowledge => self.knowledge_page(cx),
                                            Page::Appearance => self.appearance(cx),
                                        }
                                    }),
                            ),
                    ),
            )
    }
}

impl SettingsPanel {
    fn header(&self, padding: f32, cx: &mut Context<Self>) -> AnyElement {
        let mut breadcrumbs = div().flex_1().min_w_0().flex().items_center().gap_1();
        if self.page == Page::Knowledge {
            breadcrumbs = breadcrumbs.child(t("settings.knowledge.title"));
        } else if self.page == Page::Appearance {
            breadcrumbs = breadcrumbs.child(t("settings.sections.appearance"));
        } else if self.page == Page::Overview {
            breadcrumbs = breadcrumbs.child(t("settings.sections.ai"));
        } else {
            breadcrumbs = breadcrumbs
                .child(
                    Button::new("breadcrumb-ai")
                        .small()
                        .ghost()
                        .label(t("settings.sections.ai"))
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.navigate(Page::Overview, window, cx)
                        })),
                )
                .child(Icon::new(IconName::ChevronRight).size_3());
            if self.page == Page::Providers {
                breadcrumbs = breadcrumbs.child(t("settings.models.providers"));
            } else {
                let title = self
                    .editor
                    .as_ref()
                    .map(|draft| draft.display_name(cx))
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| t("native.settings.newProvider").into());
                breadcrumbs = breadcrumbs
                    .child(
                        Button::new("breadcrumb-providers")
                            .small()
                            .ghost()
                            .label(t("settings.models.providers"))
                            .disabled(self.busy())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.navigate(Page::Providers, window, cx)
                            })),
                    )
                    .child(Icon::new(IconName::ChevronRight).size_3())
                    .child(div().min_w_0().truncate().child(title));
            }
        }
        let mut header = div()
            .h(px(64.))
            .w_full()
            .max_w(px(920.))
            .mx_auto()
            .flex_shrink_0()
            .px(px(padding))
            .flex()
            .items_center()
            .gap_3()
            .text_sm()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(breadcrumbs);
        if self.page == Page::Provider {
            header = header.child(
                Button::new("settings-save")
                    .small()
                    .primary()
                    .label(t("common.save"))
                    .disabled(self.busy() || !self.dirty())
                    .on_click(cx.listener(|view, _, window, cx| view.save(window, cx))),
            );
        } else if self.page == Page::Providers {
            header = header.child(self.add_provider_button(cx));
        }
        header.into_any_element()
    }

    fn add_provider_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("add-provider")
            .small()
            .primary()
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

    fn status(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = self
            .error
            .clone()
            .or_else(|| self.preference_error.map(crate::text::translate))
            .or_else(|| self.task.as_ref().map(|task| task.to_string()))
            .or_else(|| {
                self.loading
                    .then(|| crate::text::t("native.settings.loadingModels").into_owned())
            })
            .or_else(|| self.feedback.clone());
        let Some(message) = message else {
            return div().into_any_element();
        };
        div()
            .px_5()
            .py_2()
            .flex()
            .items_center()
            .gap_3()
            .flex_shrink_0()
            .bg(cx.theme().muted)
            .text_sm()
            .text_color(if self.error.is_some() || self.preference_error.is_some() {
                cx.theme().danger
            } else {
                cx.theme().muted_foreground
            })
            .child(div().flex_1().child(message))
            .when(self.load_failed, |view| {
                view.child(
                    Button::new("settings-retry")
                        .small()
                        .ghost()
                        .label(crate::text::t("common.retry"))
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, _, cx| view.ensure_loaded(cx))),
                )
            })
            .into_any_element()
    }

    fn overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut label = crate::text::t("native.settings.noDefaultModel").into_owned();
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
        let owner = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .child(
                self.render_field(
                    crate::text::t("settings.models.defaultModel"),
                    crate::text::t("native.settings.defaultModelHint"),
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
                                            let _ = owner.update(cx, |view, cx| {
                                                if view.generation == generation {
                                                    view.set_default(selection.clone(), window, cx);
                                                }
                                            });
                                        }),
                                );
                            }
                            menu
                        }),
                    cx,
                ),
            )
            .child(
                self.render_field(
                    t("settings.models.modelProviders"),
                    t("settings.models.modelProvidersDescription"),
                    Button::new("configure-providers")
                        .label(t("settings.models.configure"))
                        .icon(IconName::ChevronRight)
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.navigate(Page::Providers, window, cx)
                        })),
                    cx,
                ),
            )
            .into_any_element()
    }

    fn providers(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .flex()
            .flex_col()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .overflow_hidden();
        let Some(catalog) = &self.catalog else {
            return content.into_any_element();
        };
        if catalog.providers.is_empty() {
            content = content.child(
                div()
                    .py_8()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.settings.providersEmpty")),
            );
        }
        for provider in &catalog.providers {
            let id = provider.config.id.clone();
            let label = provider_name(&provider.config);
            let info = crate::text::format(
                "native.settings.providerSummary",
                &[
                    ("value0", provider.config.models.len().to_string()),
                    (
                        "value1",
                        (if provider.config.authentication == LanguageModelAuthentication::None {
                            crate::text::t("settings.models.noAuthentication")
                        } else if provider.has_api_key {
                            crate::text::t("native.settings.credentialsSaved")
                        } else {
                            crate::text::t("native.settings.credentialsRequired")
                        })
                        .to_string(),
                    ),
                ],
            );
            content = content.child(
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().truncate().child(label))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(provider.config.name.clone())
                                    .child(info),
                            ),
                    )
                    .child(
                        Button::new(gpui_kit::SharedString::from(format!(
                            "provider-edit-{}",
                            provider.config.id
                        )))
                        .small()
                        .ghost()
                        .label(crate::text::t("detail.constantValue.edit"))
                        .disabled(self.busy())
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
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
                            },
                        )),
                    ),
            );
        }
        content.into_any_element()
    }
}
