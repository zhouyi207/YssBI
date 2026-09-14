//! Default Harness persistence, provider, clock and identity implementations.

use std::path::PathBuf;
use std::sync::Arc;
use yss_automation_contract::{
    AgentDriverConfigurationPort, AutomationIdKind, CapabilityGatewayPort, ClockPort,
    HarnessEventSinkPort, IdGenerationFailure, IdGeneratorPort, UnixMillis,
};
use yss_statistical_harness::{HarnessHost, HarnessPorts};

use crate::execution::ApplicationState;

pub struct HarnessTransportPorts {
    pub capability_gateway: Arc<dyn CapabilityGatewayPort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
}

pub struct HarnessServices {
    pub host: Arc<HarnessHost>,
    pub provider: Arc<dyn AgentDriverConfigurationPort>,
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessStartupError {
    #[error("Harness SQLite persistence could not be initialized")]
    Persistence(#[from] yss_automation_contract::PersistenceFailure),
    #[error("Harness application initialization failed")]
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
            AutomationIdKind::WorkflowRun => "workflow",
            AutomationIdKind::ToolInvocation => "tool",
            AutomationIdKind::CapabilityInvocation => "capability",
            AutomationIdKind::MemoryRecord => "memory",
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
    let store =
        Arc::new(yss_statistical_harness_sqlite::SqliteHarnessStore::connect(app_dir).await?);
    let provider = Arc::new(yss_agent_rig::ConfigurableAgentDriver::new());
    let host = application
        .initialize_harness(HarnessPorts {
            agent_driver: provider.clone(),
            capability_gateway: transport.capability_gateway,
            sessions: store.clone(),
            events: store.clone(),
            event_sink: transport.event_sink,
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store,
            clock: Arc::new(SystemHarnessClock),
            ids: Arc::new(HarnessIdGenerator),
        })
        .await?;
    Ok(HarnessServices { host, provider })
}
