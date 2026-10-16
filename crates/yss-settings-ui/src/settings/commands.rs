//! Async settings commands and captured save requests, shared with Save All.
use super::{SettingsPanel, models::provider_name};
use crate::SettingsServices;
use gpui_kit::{Context, Window};
use yss_application::harness::models::{CredentialChange, ModelSettingsError};
use yss_harness_contract::{
    LanguageModelAuthentication, LanguageModelCatalog, LanguageModelProviderConfig,
    LanguageModelSelection, SecretCredential,
};
use yss_ui::lifecycle::LogReleased;

pub struct SettingsSaveRequest {
    config: LanguageModelProviderConfig,
    credential: CredentialChange,
}
pub type SettingsSaveOutcome = Result<LanguageModelCatalog, ModelSettingsError>;

impl SettingsSaveRequest {
    pub fn commit(self, services: &SettingsServices) -> SettingsSaveOutcome {
        services
            .tasks
            .executor()
            .block_on(services.models.save_provider(self.config, self.credential))
    }
}

impl SettingsPanel {
    pub fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if self.catalog.is_some() || self.busy() {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.loading = true;
        let service = self.services.models.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.catalog().await });
        // The retained panel owns the read, so closing its window cannot strand loading state.
        cx.spawn(async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            view.update(cx, |view, cx| {
                if view.generation != generation {
                    return;
                }
                view.loading = false;
                match outcome {
                    Ok(catalog) => {
                        view.install_catalog(catalog);
                    }
                    Err(error) => {
                        view.report_error(failure(&error), true, cx);
                    }
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    pub fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<SettingsSaveRequest> {
        if self.busy() {
            return None;
        }
        let config = match self.configuration(cx) {
            Ok(config) => config,
            Err(error) => {
                self.report_error(error, false, cx);
                cx.notify();
                return None;
            }
        };
        let key = self.editor.as_ref().unwrap().key.read(cx).value();
        if self.replacement_key_required() && key.trim().is_empty() {
            self.report_error(
                yss_ui::text::translate("settings.models.newProviderKeyHint"),
                false,
                cx,
            );
            cx.notify();
            return None;
        }
        let credential =
            if config.authentication == LanguageModelAuthentication::None || key.is_empty() {
                CredentialChange::Keep
            } else {
                let Ok(secret) = SecretCredential::new(key.to_string()) else {
                    self.report_error(yss_ui::text::t("native.settings.emptyApiKey"), false, cx);
                    cx.notify();
                    return None;
                };
                CredentialChange::Replace(secret)
            };
        self.task = Some(yss_ui::text::t("native.settings.savingModels"));
        cx.notify();
        Some(SettingsSaveRequest { config, credential })
    }

    pub fn cancel_prepared_save(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        cx.notify();
    }

    pub fn finish_save(
        &mut self,
        outcome: SettingsSaveOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = None;
        match outcome {
            Ok(catalog) => {
                self.services.tasks.events().models_changed();
                let id = self.editor.as_ref().map(|draft| draft.id.clone());
                let saved = catalog
                    .providers
                    .iter()
                    .find(|entry| Some(&entry.config.id) == id.as_ref())
                    .cloned();
                self.install_catalog(catalog);
                self.discard(cx);
                if let Some(saved) = saved {
                    self.edit_provider(Some(saved), None, window, cx);
                }
                self.report_success(yss_ui::text::t("native.settings.modelsSaved"), cx);
            }
            Err(error) => self.report_error(failure(&error), false, cx),
        }
        cx.notify();
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty() {
            return;
        }
        let Some(request) = self.prepare_save(cx) else {
            return;
        };
        window.focus(&self.focus, cx);
        let owner = self.services.clone();
        let job = self.services.tasks.run(move |_| Ok(request.commit(&owner)));
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(Err(ModelSettingsError::Unavailable));
            view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx))
                .log_released();
        })
        .detach();
    }

    pub(super) fn discover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.connection_ready(cx) {
            return;
        }
        let Some(draft) = &self.editor else {
            return;
        };
        // Discovery validates the connection, independent of unfinished model inputs.
        let config = draft.configuration(cx);
        let key = draft.key.read(cx).value();
        let credential = if config.authentication == LanguageModelAuthentication::None
            || key.is_empty()
        {
            None
        } else {
            match SecretCredential::new(key.to_string()) {
                Ok(key) => Some(key),
                Err(_) => {
                    self.report_error(yss_ui::text::t("native.settings.emptyApiKey"), false, cx);
                    cx.notify();
                    return;
                }
            }
        };
        self.task = Some(yss_ui::text::t("native.settings.discoveringModels"));
        let epoch = self.epoch;
        let service = self.services.models.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.discover_models(config, credential).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            view.update_in(cx, |view, _, cx| {
                if view.epoch != epoch {
                    return;
                }
                view.task = None;
                match outcome {
                    Ok(models) => {
                        let editing_id = view
                            .model
                            .as_ref()
                            .map(|model| model.id.read(cx).value().trim().to_owned());
                        let mut added = 0;
                        if let Some(draft) = &mut view.editor {
                            let mut ids: std::collections::HashSet<_> =
                                draft.models.iter().map(|model| model.id.clone()).collect();
                            if let Some(id) = editing_id.filter(|id| !id.is_empty()) {
                                ids.insert(id);
                            }
                            for model in models {
                                if ids.insert(model.id.clone()) {
                                    draft.models.push(model);
                                    added += 1;
                                }
                            }
                            draft.changed |= added > 0;
                        }
                        view.report_success(
                            yss_ui::text::format(
                                "settings.models.discoveryAdded",
                                &[("count", added.to_string())],
                            ),
                            cx,
                        );
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

    pub(super) fn set_default(
        &mut self,
        selection: LanguageModelSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        self.task = Some(yss_ui::text::t("native.settings.settingDefault"));
        let service = self.services.models.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.set_default(selection).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            view.update_in(cx, |view, _, cx| {
                view.task = None;
                match outcome {
                    Ok(catalog) => {
                        view.services.tasks.events().models_changed();
                        view.install_catalog(catalog);
                        view.report_success(yss_ui::text::t("native.settings.defaultUpdated"), cx);
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

    pub(super) fn delete_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(draft) = &self.editor else {
            return;
        };
        if !draft.saved {
            return;
        }
        let id = draft.id.clone();
        let title = provider_name(&draft.configuration(cx));
        let epoch = self.epoch;
        let owner = cx.entity().downgrade();
        yss_ui::modal_window::confirm(
            yss_ui::text::t("native.settings.deleteProviderTitle"),
            yss_ui::text::format(
                "native.settings.deleteProviderMessage",
                &[("title", title.to_string())],
            ),
            yss_ui::text::t("common.delete"),
            yss_ui::text::t("common.cancel"),
            window,
            cx,
            move |_, window, cx| {
                owner
                    .update(cx, |view, cx| {
                        view.remove_provider(id.clone(), epoch, window, cx)
                    })
                    .unwrap_or(true)
            },
        );
    }

    fn remove_provider(
        &mut self,
        id: String,
        epoch: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.epoch != epoch {
            return true;
        }
        if self.busy() {
            return false;
        }
        self.task = Some(yss_ui::text::t("native.settings.deletingProvider"));
        let service = self.services.models.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.delete_provider(id).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            view.update_in(cx, |view, _, cx| {
                if view.epoch != epoch {
                    return;
                }
                view.task = None;
                match outcome {
                    Ok(catalog) => {
                        view.services.tasks.events().models_changed();
                        view.install_catalog(catalog);
                        view.discard(cx);
                        view.report_success(yss_ui::text::t("native.settings.providerDeleted"), cx);
                    }
                    Err(error) => view.report_error(failure(&error), false, cx),
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
        true
    }
}

fn failure(error: &ModelSettingsError) -> String {
    match error {
        ModelSettingsError::Invalid => yss_ui::text::t("native.settings.invalidConfiguration"),
        ModelSettingsError::Unavailable => yss_ui::text::t("native.settings.settingsUnavailable"),
        ModelSettingsError::Credentials => {
            yss_ui::text::t("native.settings.credentialsUnavailable")
        }
        ModelSettingsError::ModelUnavailable => yss_ui::text::t("native.settings.modelChanged"),
        ModelSettingsError::Provider(code) => match code {
            yss_harness_contract::AgentDriverFailureCode::ProviderAuthenticationFailed => {
                yss_ui::text::t("native.settings.authenticationFailed")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderRateLimited => {
                yss_ui::text::t("native.settings.rateLimited")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderPaymentRequired => {
                yss_ui::text::t("native.settings.paymentRequired")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderRequestRejected => {
                yss_ui::text::t("native.settings.requestRejected")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderTransportFailed => {
                yss_ui::text::t("native.settings.connectionFailed")
            }
            yss_harness_contract::AgentDriverFailureCode::DeadlineElapsed => {
                yss_ui::text::t("native.settings.timedOut")
            }
            _ => yss_ui::text::t("native.settings.serviceFailed"),
        },
    }
    .into_owned()
}
