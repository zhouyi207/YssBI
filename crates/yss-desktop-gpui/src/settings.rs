//! Native settings keep read projections and unsubmitted forms over Application services.
mod appearance;
pub(crate) mod commands;
mod fields;
mod knowledge;
mod models;
mod navigation;
mod render;

use crate::services::NativeServices;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, Subscription, Window, actions,
};
use std::sync::Arc;
use yss_harness_contract::LanguageModelCatalog;

actions!(native_settings, [SaveSettings]);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Providers,
    Provider,
    Knowledge,
    Appearance,
}

pub(crate) enum SettingsEvent {
    OpenDocument {
        project: yss_project_identity::ProjectInstanceId,
        path: String,
    },
}

impl gpui_kit::EventEmitter<SettingsEvent> for SettingsPanel {}

pub(crate) struct SettingsPanel {
    services: Arc<NativeServices>,
    focus: FocusHandle,
    catalog: Option<Arc<LanguageModelCatalog>>,
    knowledge: knowledge::KnowledgeSettings,
    page: Page,
    render_width: f32,
    editor: Option<models::ProviderDraft>,
    model: Option<models::ModelDraft>,
    provider_subscriptions: Vec<Subscription>,
    model_subscriptions: Vec<Subscription>,
    epoch: u64,
    generation: u64,
    loading: bool,
    task: Option<std::borrow::Cow<'static, str>>,
    error: Option<String>,
    preference_error: Option<&'static str>,
    load_failed: bool,
    feedback: Option<String>,
    search: Entity<InputState>,
    _search_subscription: Subscription,
}

impl SettingsPanel {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx));
        let search_subscription = cx.subscribe_in(&search, window, |view, _, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                let categories = view.visible_categories(cx);
                let current_visible = categories
                    .iter()
                    .any(|(page, _, _)| page.category() == view.page.category());
                if !current_visible
                    && !view.dirty()
                    && !view.busy()
                    && let Some((page, _, _)) = categories.first()
                {
                    view.navigate(*page, window, cx);
                }
                cx.notify();
            }
        });
        let preference_error = services
            .preference_load_failed
            .then_some("native.settings.preferencesReadFailed");
        Self {
            services,
            focus: cx.focus_handle(),
            catalog: None,
            knowledge: knowledge::KnowledgeSettings::default(),
            page: Page::Overview,
            render_width: 1000.,
            editor: None,
            model: None,
            provider_subscriptions: vec![],
            model_subscriptions: vec![],
            epoch: 0,
            generation: 0,
            loading: false,
            task: None,
            error: None,
            preference_error,
            load_failed: false,
            feedback: None,
            search,
            _search_subscription: search_subscription,
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.loading || self.has_pending_operation()
    }

    pub(crate) fn has_pending_operation(&self) -> bool {
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
