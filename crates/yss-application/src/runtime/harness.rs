//! Default Harness persistence, provider, clock and identity implementations.

use std::path::PathBuf;
use std::sync::Arc;
use yss_assistant_contract::CapabilityGatewayPort;
use yss_harness_contract::{
    AutomationIdKind, ClockPort, HarnessEventSinkPort, IdGeneratorPort, UnixMillis,
};
use yss_harness_core::{HarnessHost, HarnessPorts};

use crate::session::ApplicationState;

pub struct HarnessTransportPorts {
    pub capability_gateway: Arc<dyn CapabilityGatewayPort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
}

pub struct HarnessServices {
    pub host: Arc<HarnessHost>,
    pub models: Arc<crate::harness::models::LanguageModelService>,
    pub knowledge: Arc<crate::harness::knowledge::ProjectKnowledgeService>,
    pub web: Arc<crate::harness::web::WebSearchService>,
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessStartupError {
    #[error("Harness SQLite persistence could not be initialized: {0}")]
    Persistence(#[from] yss_harness_contract::PersistenceFailure),
    #[error("Harness application initialization failed: {0}")]
    Application(#[from] crate::harness::HarnessInitializationError),
    #[error("Assistant extension could not be initialized: {0}")]
    Extension(#[from] yss_assistant_extension::AssistantExtensionError),
    #[error("Web access could not be initialized: {0}")]
    Web(#[from] crate::harness::web::WebSearchSettingsError),
}

struct SystemHarnessClock;

impl ClockPort for SystemHarnessClock {
    fn now(&self) -> UnixMillis {
        let milliseconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        UnixMillis::from_existing(u64::try_from(milliseconds).unwrap_or(u64::MAX))
    }
}

struct HarnessIdGenerator;

impl IdGeneratorPort for HarnessIdGenerator {
    fn next_id(&self, kind: AutomationIdKind) -> String {
        format!("{}-{}", kind.prefix(), uuid::Uuid::new_v4())
    }
}

pub(super) async fn initialize(
    app_dir: PathBuf,
    application: &ApplicationState,
    transport: HarnessTransportPorts,
) -> Result<HarnessServices, HarnessStartupError> {
    let store = Arc::new(yss_harness_sqlite::SqliteHarnessStore::connect(app_dir.clone()).await?);
    let python_dir = app_dir.clone();
    let web = Arc::new(crate::harness::web::WebSearchService::new(
        app_dir.clone(),
        Arc::new(crate::harness::models::SystemModelCredentials),
    )?);
    let models = Arc::new(crate::harness::models::LanguageModelService::new(
        app_dir,
        Arc::new(crate::harness::models::SystemModelCredentials),
    ));
    let clock = Arc::new(SystemHarnessClock);
    let knowledge = Arc::new(crate::harness::knowledge::ProjectKnowledgeService::new(
        application.clone(),
        store.clone(),
        clock.clone(),
    ));
    let extension = Arc::new(yss_assistant_extension::AssistantExtension::new(
        transport.capability_gateway,
    )?);
    let host = application
        .initialize_harness(HarnessPorts {
            models: models.clone(),
            resources: Arc::new(crate::harness::ApplicationResourceResolver(
                application.clone(),
            )),
            extension,
            sessions: store.clone(),
            events: store.clone(),
            event_sink: transport.event_sink,
            tool_ledger: store.clone(),
            knowledge: knowledge.clone(),
            knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            web: web.port(),
            python: Arc::new(crate::harness::python::ApplicationPython::new(
                application.clone(),
                python_dir,
            )),
            clock,
            ids: Arc::new(HarnessIdGenerator),
        })
        .await?;
    Ok(HarnessServices {
        host,
        models,
        knowledge,
        web,
    })
}
