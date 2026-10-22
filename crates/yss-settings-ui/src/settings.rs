//! Settings retain read projections and unsubmitted forms over canonical domain services.
mod commands;
mod fields;
mod keybindings;
mod knowledge;
mod models;
mod preference_fields;
mod preferences;
mod render;

pub use commands::{SettingsSaveOutcome, SettingsSaveRequest};

use crate::SettingsServices;
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

pub enum SettingsEvent {
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

pub struct SettingsPanel {
    services: Arc<SettingsServices>,
    focus: FocusHandle,
    breadcrumb_focus: FocusHandle,
    catalog: Option<Arc<LanguageModelCatalog>>,
    knowledge: knowledge::KnowledgeSettings,
    notifications: yss_ui::notifications::Notifications,
    page: Page,
    initial_page: gpui_kit::component::setting::SelectIndex,
    // Settings keeps its selected page in keyed state; a new key applies `initial_page`.
    navigation: u64,
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
    pub fn new(
        services: Arc<SettingsServices>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let preference_error = services
            .preference_load_failed
            .then_some("native.settings.preferencesReadFailed");
        Self {
            services,
            focus: cx.focus_handle(),
            breadcrumb_focus: cx.focus_handle().tab_stop(true),
            catalog: None,
            knowledge: knowledge::KnowledgeSettings::default(),
            notifications: yss_ui::notifications::Notifications::new(window),
            page: Page::Overview,
            initial_page: Default::default(),
            navigation: 0,
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

    fn busy(&self) -> bool {
        self.loading || self.has_pending_operation()
    }

    pub fn has_pending_operation(&self) -> bool {
        self.task.is_some()
            || self.knowledge.pending
            || self.preference_timer.is_some()
            || self.preference_write.is_some()
    }

    pub fn show_keybindings(&mut self, cx: &mut Context<Self>) {
        self.initial_page = gpui_kit::component::setting::SelectIndex {
            page_ix: render::KEYBINDINGS_PAGE,
            group_ix: None,
        };
        self.navigation = self.navigation.wrapping_add(1);
        cx.notify();
    }

    pub fn set_theme_mode(&mut self, mode: yss_settings::ThemeMode, cx: &mut Context<Self>) {
        self.save_preferences(move |settings| settings.appearance.mode = mode, cx);
    }

    pub fn dirty(&self) -> bool {
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

    pub fn report_preference_error(&self, cx: &mut Context<Self>) {
        if let Some(key) = self.preference_error {
            self.report_error(yss_ui::text::translate(key), false, cx);
        }
    }

    fn install_catalog(&mut self, catalog: LanguageModelCatalog) {
        self.generation = self.generation.wrapping_add(1);
        self.catalog = Some(Arc::new(catalog));
    }

    pub fn discard(&mut self, cx: &mut Context<Self>) {
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
