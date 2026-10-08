//! Async settings commands and captured save requests, shared with Save All.
use super::{SettingsPanel, models::provider_name};
use crate::services::NativeServices;
use gpui::{Context, Window};
use gpui_component::{WindowExt, button::ButtonVariant};
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
    pub(crate) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.task = Some("读取模型配置…");
        self.load_failed = false;
        let service = self.services.application.harness.models.clone();
        let job = self
            .services
            .executor
            .spawn(async move { service.catalog().await });
        cx.spawn_in(window, async move |view, cx| {
            let outcome = job.await.unwrap_or(Err(ModelSettingsError::Unavailable));
            let _ = view.update_in(cx, |view, _, cx| {
                if view.generation != generation {
                    return;
                }
                view.task = None;
                match outcome {
                    Ok(catalog) => {
                        view.install_catalog(catalog);
                        view.error = None;
                    }
                    Err(error) => {
                        view.error = Some(failure(&error));
                        view.load_failed = true;
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
                self.error = Some(error);
                cx.notify();
                return None;
            }
        };
        let key = self.editor.as_ref().unwrap().key.read(cx).value();
        if self.replacement_key_required() && key.trim().is_empty() {
            self.error = Some(crate::text::translate("settings.models.newProviderKeyHint"));
            cx.notify();
            return None;
        }
        let credential =
            if config.authentication == LanguageModelAuthentication::None || key.is_empty() {
                CredentialChange::Keep
            } else {
                let Ok(secret) = SecretCredential::new(key.to_string()) else {
                    self.error = Some("API Key 不能为空白。".into());
                    cx.notify();
                    return None;
                };
                CredentialChange::Replace(secret)
            };
        self.task = Some("保存模型配置…");
        self.error = None;
        self.load_failed = false;
        self.feedback = None;
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
                self.feedback = Some("模型配置已保存。".into());
            }
            Err(error) => self.error = Some(failure(&error)),
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
                        self.error = Some("API Key 不能为空白。".into());
                        cx.notify();
                        return;
                    }
                }
            };
        self.task = Some("获取服务端模型…");
        self.error = None;
        self.load_failed = false;
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
                        view.feedback = Some(crate::text::format(
                            "settings.models.discoveryAdded",
                            &[("count", added.to_string())],
                        ));
                    }
                    Err(error) => view.error = Some(failure(&error)),
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
        self.task = Some("设置默认模型…");
        self.error = None;
        self.load_failed = false;
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
                        view.feedback = Some("默认模型已更新。".into());
                    }
                    Err(error) => view.error = Some(failure(&error)),
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
        window.open_alert_dialog(cx, move |alert, _, _| {
            let owner = owner.clone();
            let id = id.clone();
            alert
                .title("删除供应商？")
                .description(format!("将删除“{title}”的配置和保存的凭据；历史对话保留。"))
                .confirm()
                .ok_text("删除")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("取消")
                .on_ok(move |_, window, cx| {
                    owner
                        .update(cx, |view, cx| {
                            view.remove_provider(id.clone(), epoch, window, cx)
                        })
                        .unwrap_or(true)
                })
        });
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
        self.task = Some("删除供应商…");
        self.error = None;
        self.load_failed = false;
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
                        view.feedback = Some("供应商已删除。".into());
                    }
                    Err(error) => view.error = Some(failure(&error)),
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
        ModelSettingsError::Invalid => "配置无效，请检查连接地址、协议和模型参数。",
        ModelSettingsError::Unavailable => "模型配置暂不可用，请检查应用数据目录。",
        ModelSettingsError::Credentials => "系统凭据库不可用，请检查系统密钥环后重试。",
        ModelSettingsError::ModelUnavailable => "模型或保存的连接已变化，请重新选择并检查凭据。",
        ModelSettingsError::Provider(code) => match code {
            yss_harness_contract::AgentDriverFailureCode::ProviderAuthenticationFailed => {
                "服务端拒绝认证，请检查 API Key 和认证方式。"
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderRateLimited => {
                "服务端请求达到限额，请稍后再试。"
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderPaymentRequired => {
                "服务端账户额度不足，请检查账户余额。"
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderRequestRejected => {
                "服务端拒绝请求，请检查协议和 API 根地址。"
            }
            yss_harness_contract::AgentDriverFailureCode::ProviderTransportFailed => {
                "未能连接服务端，请检查网络和连接地址。"
            }
            yss_harness_contract::AgentDriverFailureCode::DeadlineElapsed => {
                "服务端请求超时，请稍后再试。"
            }
            _ => "服务端请求未完成，请检查连接和服务端状态。",
        },
    }
    .to_owned()
}
