//! Async settings commands and captured save requests, shared with Save All.
use super::{SettingsPanel, models::provider_name};
use crate::services::NativeServices;
use gpui_kit::{Context, Window};
use std::sync::Arc;
use yss_application::{
    harness::models::{CredentialChange, ModelSettingsError},
    runtime::ApplicationServices,
};
use yss_harness_contract::{
    LanguageModelAuthentication, LanguageModelCatalog, LanguageModelProviderConfig,
    LanguageModelSelection, SecretCredential,
};

pub(crate) struct SettingsSaveRequest {
    config: LanguageModelProviderConfig,
    credential: CredentialChange,
}
pub(crate) type SettingsSaveOutcome = Result<LanguageModelCatalog, ModelSettingsError>;

impl SettingsSaveRequest {
    pub fn commit(
        self,
        services: &ApplicationServices,
        owner: &Arc<NativeServices>,
    ) -> SettingsSaveOutcome {
        owner.executor.block_on(
            services
                .harness
                .models
                .save_provider(self.config, self.credential),
        )
    }
}

impl SettingsPanel {
    pub(crate) fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if self.catalog.is_some() || self.busy() {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.loading = true;
        let service = self.services.application.harness.models.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.catalog().await });
        // The retained panel owns the read, so closing its window cannot strand loading state.
        cx.spawn(async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            let _ = view.update(cx, |view, cx| {
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
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn prepare_save(&mut self, cx: &mut Context<Self>) -> Option<SettingsSaveRequest> {
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
                crate::text::translate("settings.models.newProviderKeyHint"),
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
                    self.report_error(crate::text::t("native.settings.emptyApiKey"), false, cx);
                    cx.notify();
                    return None;
                };
                CredentialChange::Replace(secret)
            };
        self.task = Some(crate::text::t("native.settings.savingModels"));
        cx.notify();
        Some(SettingsSaveRequest { config, credential })
    }

    pub(crate) fn cancel_prepared_save(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        cx.notify();
    }

    pub(crate) fn finish_save(
        &mut self,
        outcome: SettingsSaveOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = None;
        match outcome {
            Ok(catalog) => {
                self.services.models_changed();
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
                self.report_success(crate::text::t("native.settings.modelsSaved"), cx);
            }
            Err(error) => self.report_error(failure(&error), false, cx),
        }
        cx.notify();
    }

    pub(crate) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty() {
            return;
        }
        let Some(request) = self.prepare_save(cx) else {
            return;
        };
        window.focus(&self.focus, cx);
        let owner = self.services.clone();
        let job = self
            .services
            .run(move |services| Ok(request.commit(services, &owner)));
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(Err(ModelSettingsError::Unavailable));
            let _ = view.update_in(cx, |view, window, cx| view.finish_save(outcome, window, cx));
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
        let credential =
            if config.authentication == LanguageModelAuthentication::None || key.is_empty() {
                None
            } else {
                match SecretCredential::new(key.to_string()) {
                    Ok(key) => Some(key),
                    Err(_) => {
                        self.report_error(crate::text::t("native.settings.emptyApiKey"), false, cx);
                        cx.notify();
                        return;
                    }
                }
            };
        self.task = Some(crate::text::t("native.settings.discoveringModels"));
        let epoch = self.epoch;
        let service = self.services.application.harness.models.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.discover_models(config, credential).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            let _ = view.update_in(cx, |view, _, cx| {
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
                            crate::text::format(
                                "settings.models.discoveryAdded",
                                &[("count", added.to_string())],
                            ),
                            cx,
                        );
                    }
                    Err(error) => view.report_error(failure(&error), false, cx),
                }
                cx.notify();
            });
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
        self.task = Some(crate::text::t("native.settings.settingDefault"));
        let service = self.services.application.harness.models.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.set_default(selection).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            let _ = view.update_in(cx, |view, _, cx| {
                view.task = None;
                match outcome {
                    Ok(catalog) => {
                        view.services.models_changed();
                        view.install_catalog(catalog);
                        view.report_success(crate::text::t("native.settings.defaultUpdated"), cx);
                    }
                    Err(error) => view.report_error(failure(&error), false, cx),
                }
                cx.notify();
            });
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
        crate::modal_window::confirm(
            crate::text::t("native.settings.deleteProviderTitle"),
            crate::text::format(
                "native.settings.deleteProviderMessage",
                &[("title", title.to_string())],
            ),
            crate::text::t("common.delete"),
            crate::text::t("common.cancel"),
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
        self.task = Some(crate::text::t("native.settings.deletingProvider"));
        let service = self.services.application.harness.models.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.delete_provider(id).await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            let _ = view.update_in(cx, |view, _, cx| {
                if view.epoch != epoch {
                    return;
                }
                view.task = None;
                match outcome {
                    Ok(catalog) => {
                        view.services.models_changed();
                        view.install_catalog(catalog);
                        view.discard(cx);
                        view.report_success(crate::text::t("native.settings.providerDeleted"), cx);
                    }
                    Err(error) => view.report_error(failure(&error), false, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        true
    }
}

fn failure(error: &ModelSettingsError) -> String {
    match error {
        ModelSettingsError::Invalid => crate::text::t("native.settings.invalidConfiguration"),
        ModelSettingsError::Unavailable => crate::text::t("native.settings.settingsUnavailable"),
        ModelSettingsError::Credentials => crate::text::t("native.settings.credentialsUnavailable"),
        ModelSettingsError::ModelUnavailable => crate::text::t("native.settings.modelChanged"),
        ModelSettingsError::Provider(code) => match code {
            yss_harness_contract::AgentDriverFailureCode::ProviderAuthenticationFailed => {
                crate::text::t("native.settings.authenticationFailed")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderRateLimited => {
                crate::text::t("native.settings.rateLimited")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderPaymentRequired => {
                crate::text::t("native.settings.paymentRequired")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderRequestRejected => {
                crate::text::t("native.settings.requestRejected")
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderTransportFailed => {
                crate::text::t("native.settings.connectionFailed")
            }
            yss_harness_contract::AgentDriverFailureCode::DeadlineElapsed => {
                crate::text::t("native.settings.timedOut")
            }
            _ => crate::text::t("native.settings.serviceFailed"),
        },
    }
    .into_owned()
}
