//! Web search provider form over the Application web search settings.
use super::SettingsPanel;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    setting::{SettingGroup, SettingItem},
};
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div};
use yss_application::harness::{
    models::CredentialChange,
    web::{WebSearchProvider, WebSearchSettingsError, WebSearchStatus},
};
use yss_harness_contract::SecretCredential;
use yss_ui::{lifecycle::LogReleased, text::t};

pub(super) struct WebSearchForm {
    provider: WebSearchProvider,
    key: Entity<InputState>,
    address: Entity<InputState>,
    status: Option<WebSearchStatus>,
    // The saved address is copied into the input once, by the first render with a window.
    address_applied: bool,
    generation: u64,
    loading: bool,
    pub(super) pending: bool,
}

impl WebSearchForm {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SettingsPanel>) -> Self {
        Self {
            provider: WebSearchProvider::Off,
            key: cx.new(|cx| InputState::new(window, cx).masked(true)),
            address: cx
                .new(|cx| InputState::new(window, cx).placeholder("https://searx.example.org")),
            status: None,
            address_applied: false,
            generation: 0,
            loading: false,
            pending: false,
        }
    }
}

fn provider_label(provider: WebSearchProvider) -> String {
    t(match provider {
        WebSearchProvider::Off => "settings.webSearch.off",
        WebSearchProvider::Brave => "settings.webSearch.brave",
        WebSearchProvider::Tavily => "settings.webSearch.tavily",
        WebSearchProvider::Searxng => "settings.webSearch.searxng",
    })
    .into_owned()
}

fn failure(error: &WebSearchSettingsError) -> String {
    t(match error {
        WebSearchSettingsError::MissingApiKey => "settings.webSearch.missingKey",
        WebSearchSettingsError::InvalidAddress => "settings.webSearch.invalidAddress",
        WebSearchSettingsError::Credentials => "native.settings.credentialsUnavailable",
        WebSearchSettingsError::Unavailable => "native.settings.settingsUnavailable",
    })
    .into_owned()
}

impl SettingsPanel {
    pub(super) fn ensure_web_search_loaded(&mut self, cx: &mut Context<Self>) {
        let form = &mut self.web_search;
        if form.status.is_some() || form.loading || form.pending {
            return;
        }
        form.generation = form.generation.wrapping_add(1);
        let generation = form.generation;
        form.loading = true;
        let service = self.services.web.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.status().await });
        cx.spawn(async move |view, cx| {
            let outcome = job
                .await
                .unwrap_or(Err(WebSearchSettingsError::Unavailable));
            view.update(cx, |view, cx| {
                if view.web_search.generation != generation {
                    return;
                }
                view.web_search.loading = false;
                match outcome {
                    Ok(status) => view.install_web_search(status),
                    Err(error) => view.report_error(failure(&error), false, cx),
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
    }

    fn install_web_search(&mut self, status: WebSearchStatus) {
        self.web_search.provider = status.provider;
        self.web_search.status = Some(status);
        self.web_search.address_applied = false;
    }

    /// Copies loaded values into the retained inputs; inputs need a window to change.
    pub(super) fn sync_web_search_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let form = &mut self.web_search;
        let Some(status) = &form.status else {
            return;
        };
        if !form.address_applied {
            form.address_applied = true;
            let address = status.searxng_url.clone();
            form.address
                .update(cx, |input, cx| input.set_value(address, window, cx));
        }
        let placeholder = if status.has_api_key && status.provider == form.provider {
            "********".to_owned()
        } else {
            t("settings.models.enterKey").into_owned()
        };
        if form.key.read(cx).presentation().placeholder().as_ref() != placeholder.as_str() {
            form.key.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
    }

    fn select_web_search_provider(&mut self, provider: WebSearchProvider, cx: &mut Context<Self>) {
        self.web_search.provider = provider;
        cx.notify();
    }

    fn save_web_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let form = &mut self.web_search;
        if form.pending || form.loading {
            return;
        }
        let key = form.key.read(cx).value().trim().to_owned();
        let api_key = if key.is_empty() {
            CredentialChange::Keep
        } else {
            match SecretCredential::new(key) {
                Ok(credential) => CredentialChange::Replace(credential),
                Err(_) => {
                    self.report_error(failure(&WebSearchSettingsError::MissingApiKey), false, cx);
                    return;
                }
            }
        };
        let provider = form.provider;
        let address = form.address.read(cx).value().to_string();
        form.pending = true;
        form.generation = form.generation.wrapping_add(1);
        let generation = form.generation;
        let service = self.services.web.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.save(provider, address, api_key).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .unwrap_or(Err(WebSearchSettingsError::Unavailable));
            view.update_in(cx, |view, window, cx| {
                view.web_search.pending = false;
                if view.web_search.generation != generation {
                    return;
                }
                match outcome {
                    Ok(status) => {
                        view.install_web_search(status);
                        view.web_search
                            .key
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        view.report_success(t("settings.webSearch.saved"), cx);
                    }
                    Err(error) => view.report_error(failure(&error), false, cx),
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn web_search_group(&self, cx: &mut Context<Self>) -> SettingGroup {
        let provider = self.web_search.provider;
        let mut group = SettingGroup::new()
            .title(t("settings.webSearch.title"))
            .item(
                self.render_field(
                    t("settings.webSearch.provider"),
                    t("settings.webSearch.providerHint"),
                    |view, cx| view.web_search_provider_button(cx),
                    cx,
                )
                .keywords([t("settings.webSearch.title")]),
            );
        if provider.requires_api_key() {
            group = group.item(
                self.render_field(
                    t("settings.models.apiKey"),
                    t("settings.webSearch.keyHint"),
                    |view, _| Input::new(&view.web_search.key).mask_toggle(),
                    cx,
                )
                .keywords([t("settings.webSearch.title")]),
            );
        }
        if provider == WebSearchProvider::Searxng {
            group = group.item(
                self.render_field(
                    t("settings.webSearch.address"),
                    t("settings.webSearch.addressHint"),
                    |view, _| Input::new(&view.web_search.address),
                    cx,
                )
                .keywords([t("settings.webSearch.title")]),
            );
        }
        let owner = cx.weak_entity();
        group.item(SettingItem::render(move |_, _, cx| {
            owner
                .update(cx, |view, cx| {
                    div()
                        .flex()
                        .child(
                            Button::new("web-search-save")
                                .small()
                                .primary()
                                .label(t("settings.webSearch.save"))
                                .loading(view.web_search.pending)
                                .disabled(view.web_search.loading || view.web_search.pending)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.save_web_search(window, cx)
                                })),
                        )
                        .into_any_element()
                })
                .unwrap_or_else(|_| div().into_any_element())
        }))
    }

    fn web_search_provider_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let owner = cx.weak_entity();
        let selected = self.web_search.provider;
        div().flex().justify_end().child(
            Button::new("web-search-provider")
                .min_w_0()
                .label(provider_label(selected))
                .disabled(self.web_search.loading || self.web_search.pending)
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for provider in WebSearchProvider::ALL {
                        let owner = owner.clone();
                        menu = menu.item(
                            PopupMenuItem::new(provider_label(provider))
                                .checked(provider == selected)
                                .on_click(move |_, _, cx| {
                                    if let Err(error) = owner.update(cx, |view, cx| {
                                        view.select_web_search_provider(provider, cx)
                                    }) {
                                        tracing::debug!(%error, "Settings view closed");
                                    }
                                }),
                        );
                    }
                    menu
                }),
        )
    }
}
