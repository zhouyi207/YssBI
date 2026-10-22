mod settings;

pub use settings::{
    BackInSettings, SaveSettings, SettingsEvent, SettingsPanel, SettingsSaveOutcome,
    SettingsSaveRequest,
};

use std::sync::Arc;
use yss_application::harness::{
    knowledge::ProjectKnowledgeService, models::LanguageModelService, web::WebSearchService,
};
use yss_desktop_services::ApplicationTasks;
use yss_settings::SettingsStore;

/// Settings capabilities supplied by the application composition root.
/// The panel retains drafts; these handles retain the canonical service owners.
pub struct SettingsServices {
    tasks: Arc<ApplicationTasks>,
    models: Arc<LanguageModelService>,
    knowledge: Arc<ProjectKnowledgeService>,
    web: Arc<WebSearchService>,
    preferences: Arc<SettingsStore>,
    preference_load_failed: bool,
    logs: Option<yss_logging::LogRuntime>,
}

impl SettingsServices {
    pub fn new(
        tasks: Arc<ApplicationTasks>,
        models: Arc<LanguageModelService>,
        knowledge: Arc<ProjectKnowledgeService>,
        web: Arc<WebSearchService>,
        preferences: Arc<SettingsStore>,
        preference_load_failed: bool,
        logs: Option<yss_logging::LogRuntime>,
    ) -> Self {
        Self {
            tasks,
            models,
            knowledge,
            web,
            preferences,
            preference_load_failed,
            logs,
        }
    }
}
