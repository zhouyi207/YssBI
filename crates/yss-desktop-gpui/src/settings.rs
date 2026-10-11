//! Native settings keep read projections and unsubmitted forms over Application services.
pub(crate) mod commands;
mod fields;
mod keybindings;
mod knowledge;
mod models;
mod preference_fields;
mod preferences;
mod render;

use crate::services::NativeServices;
use gpui_kit::{App, Context, FocusHandle, Focusable, Subscription, Window, actions};
use std::sync::Arc;
use yss_harness_contract::LanguageModelCatalog;

actions!(native_settings, [SaveSettings, BackInSettings]);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Providers,
    Provider,
}

pub(crate) enum SettingsEvent {
    OpenDocument {
        project: yss_project_identity::ProjectInstanceId,
        path: String,
    },
    Error {
        message: String,
        retry_catalog: bool,
    },
    Success(String),
}

impl gpui_kit::EventEmitter<SettingsEvent> for SettingsPanel {}

pub(crate) struct SettingsPanel {
    services: Arc<NativeServices>,
    focus: FocusHandle,
    catalog: Option<Arc<LanguageModelCatalog>>,
    knowledge: knowledge::KnowledgeSettings,
    page: Page,
    initial_page: gpui_kit::component::setting::SelectIndex,
    editor: Option<models::ProviderDraft>,
    model: Option<models::ModelDraft>,
    provider_subscriptions: Vec<Subscription>,
    model_subscriptions: Vec<Subscription>,
    epoch: u64,
    generation: u64,
    loading: bool,
    task: Option<std::borrow::Cow<'static, str>>,
    preference_error: Option<&'static str>,
    preference_draft: Option<yss_settings::UserSettings>,
    preference_revision: u64,
    preference_timer: Option<gpui_kit::Task<()>>,
    preference_write: Option<gpui_kit::Task<()>>,
    font_names: Arc<[gpui_kit::SharedString]>,
}

impl SettingsPanel {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let preference_error = services
            .preference_load_failed
            .then_some("native.settings.preferencesReadFailed");
        Self {
            services,
            focus: cx.focus_handle(),
            catalog: None,
            knowledge: knowledge::KnowledgeSettings::default(),
            page: Page::Overview,
            initial_page: Default::default(),
            editor: None,
            model: None,
            provider_subscriptions: vec![],
            model_subscriptions: vec![],
            epoch: 0,
            generation: 0,
            loading: false,
            task: None,
            preference_error,
            preference_draft: None,
            preference_revision: 0,
            preference_timer: None,
            preference_write: None,
            font_names: {
                let mut names = cx.text_system().all_font_names();
                names.sort();
                names.dedup();
                names.into_iter().map(Into::into).collect()
            },
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.loading || self.has_pending_operation()
    }

    pub(crate) fn has_pending_operation(&self) -> bool {
        self.task.is_some()
            || self.knowledge.pending
            || self.preference_timer.is_some()
            || self.preference_write.is_some()
    }

    pub(crate) fn dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(|draft| draft.changed)
            || self.model.as_ref().is_some_and(|draft| draft.changed)
    }

    fn report_error(
        &self,
        message: impl Into<String>,
        retry_catalog: bool,
        cx: &mut Context<Self>,
    ) {
        let message = message.into();
        tracing::warn!(code = "native_settings_failed", %message, "Settings operation failed");
        cx.emit(SettingsEvent::Error {
            message,
            retry_catalog,
        });
    }

    fn report_success(&self, message: impl Into<String>, cx: &mut Context<Self>) {
        let message = message.into();
        tracing::info!(code = "native_settings_completed", %message, "Settings operation completed");
        cx.emit(SettingsEvent::Success(message));
    }

    pub(crate) fn report_preference_error(&self, cx: &mut Context<Self>) {
        if let Some(key) = self.preference_error {
            self.report_error(crate::text::translate(key), false, cx);
        }
    }

    fn install_catalog(&mut self, catalog: LanguageModelCatalog) {
        self.generation = self.generation.wrapping_add(1);
        self.catalog = Some(Arc::new(catalog));
    }

    pub(crate) fn discard(&mut self, cx: &mut Context<Self>) {
        self.epoch = self.epoch.wrapping_add(1);
        self.editor = None;
        self.model = None;
        self.provider_subscriptions.clear();
        self.model_subscriptions.clear();
        self.page = Page::Providers;
        cx.notify();
    }
}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
