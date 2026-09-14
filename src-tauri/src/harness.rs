//! Desktop implementations injected into the application's Harness lifecycle.

use std::sync::Arc;
use yss_automation_contract::{
    AutomationIdKind, ClockPort, IdGenerationFailure, IdGeneratorPort, UnixMillis,
};

#[derive(Debug, thiserror::Error)]
pub(super) enum HarnessStartupError {
    #[error("Harness SQLite persistence could not be initialized")]
    Persistence(#[from] yss_automation_contract::PersistenceFailure),
    #[error("Harness application initialization failed")]
    Application(#[from] yss_application::harness::HarnessInitializationError),
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
    app_dir: std::path::PathBuf,
    application: yss_application::execution::ApplicationState,
) -> Result<yss_ipc_command::HarnessRuntimeState, HarnessStartupError> {
    let store =
        Arc::new(yss_statistical_harness_sqlite::SqliteHarnessStore::connect(app_dir).await?);
    let channels = Arc::new(yss_ipc_channel::HarnessChannelHub::new());
    let graph_clients = Arc::new(yss_ipc_channel::HarnessGraphClientHub::new());
    let agent_driver = Arc::new(yss_agent_rig::ConfigurableAgentDriver::new());
    let host = application
        .initialize_harness(yss_statistical_harness::HarnessPorts {
            agent_driver: agent_driver.clone(),
            capability_gateway: Arc::new(yss_ipc_command::ApplicationCapabilityGateway::new(
                application.clone(),
                graph_clients.clone(),
            )),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: channels.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store,
            clock: Arc::new(SystemHarnessClock),
            ids: Arc::new(HarnessIdGenerator),
        })
        .await?;
    Ok(yss_ipc_command::HarnessRuntimeState::new(
        host,
        channels,
        agent_driver,
        graph_clients,
    ))
}
