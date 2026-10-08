//! Native settings keep read projections and unsubmitted forms over Application services.
pub(crate) mod commands;
mod fields;
mod knowledge;
mod models;
mod render;

use crate::services::NativeServices;
use gpui::{App, Context, FocusHandle, Focusable, Subscription, actions};
use std::sync::Arc;
use yss_harness_contract::LanguageModelCatalog;

actions!(native_settings, [SaveSettings]);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Providers,
    Provider,
    Knowledge,
}

pub(crate) enum SettingsEvent {
    OpenDocument {
        project: yss_project_identity::ProjectInstanceId,
        path: String,
    },
}

impl gpui::EventEmitter<SettingsEvent> for SettingsPanel {}

pub(crate) struct SettingsPanel {
    services: Arc<NativeServices>,
    focus: FocusHandle,
    catalog: Option<Arc<LanguageModelCatalog>>,
    page: Page,
    render_width: f32,
    knowledge: knowledge::KnowledgeSettings,
    editor: Option<models::ProviderDraft>,
    model: Option<models::ModelDraft>,
    provider_subscriptions: Vec<Subscription>,
    model_subscriptions: Vec<Subscription>,
    epoch: u64,
    generation: u64,
    task: Option<&'static str>,
    error: Option<String>,
    load_failed: bool,
    feedback: Option<String>,
}

impl SettingsPanel {
    pub(crate) fn new(services: Arc<NativeServices>, cx: &mut Context<Self>) -> Self {
        Self {
            services,
            focus: cx.focus_handle(),
            catalog: None,
            page: Page::Overview,
            render_width: 1000.,
            knowledge: knowledge::KnowledgeSettings::default(),
            editor: None,
            model: None,
            provider_subscriptions: vec![],
            model_subscriptions: vec![],
            epoch: 0,
            generation: 0,
            task: None,
            error: None,
            load_failed: false,
            feedback: None,
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.task.is_some() || self.knowledge.pending
    }

    pub(crate) fn dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(|draft| draft.changed)
            || self.model.as_ref().is_some_and(|draft| draft.changed)
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
        self.error = None;
        self.feedback = None;
        cx.notify();
    }
}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
