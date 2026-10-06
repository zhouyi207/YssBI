//! Default Harness persistence, provider, clock and identity implementations.

use std::path::PathBuf;
use std::sync::Arc;
use yss_harness_contract::{
    AutomationIdKind, CapabilityGatewayPort, ClockPort, HarnessEventSinkPort, IdGenerationFailure,
    IdGeneratorPort, UnixMillis,
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
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessStartupError {
    #[error("Harness SQLite persistence could not be initialized: {0}")]
    Persistence(#[from] yss_harness_contract::PersistenceFailure),
    #[error("Harness application initialization failed: {0}")]
    Application(#[from] crate::harness::HarnessInitializationError),
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
    fn next_id(&self, kind: AutomationIdKind) -> Result<String, IdGenerationFailure> {
        let prefix = match kind {
            AutomationIdKind::HarnessSession => "session",
            AutomationIdKind::HarnessTurn => "turn",
            AutomationIdKind::AgentRun => "agent",
            AutomationIdKind::WorkflowRun => "workflow",
            AutomationIdKind::ToolInvocation => "tool",
            AutomationIdKind::ApprovalGrant => "approval",
        };
        Ok(format!("{prefix}-{}", uuid::Uuid::new_v4()))
    }
}

pub(super) async fn initialize(
    app_dir: PathBuf,
    application: &ApplicationState,
    transport: HarnessTransportPorts,
) -> Result<HarnessServices, HarnessStartupError> {
    let store = Arc::new(yss_harness_sqlite::SqliteHarnessStore::connect(app_dir.clone()).await?);
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
    let host = application
        .initialize_harness(HarnessPorts {
            models: models.clone(),
            resources: Arc::new(crate::harness::ApplicationResourceResolver(
                application.clone(),
            )),
            capability_gateway: transport.capability_gateway,
            sessions: store.clone(),
            events: store.clone(),
            event_sink: transport.event_sink,
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: knowledge.clone(),
            knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
            approvals: store,
            clock,
            ids: Arc::new(HarnessIdGenerator),
        })
        .await?;
    Ok(HarnessServices {
        host,
        models,
        knowledge,
    })
}
