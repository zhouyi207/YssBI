use std::sync::Arc;
use yss_harness_contract::{
    AgentDriverPort, ApprovalStorePort, ClockPort, HarnessEventSinkPort, HarnessEventStorePort,
    HarnessSessionStorePort, IdGeneratorPort, KnowledgeSourceStorePort, MemoryStorePort,
    ToolInvocationLedgerPort, WorkflowStorePort,
};

#[derive(Clone)]
pub struct HarnessPorts {
    pub agent_driver: Arc<dyn AgentDriverPort>,
    pub capability_gateway: Arc<dyn yss_harness_contract::CapabilityGatewayPort>,
    pub sessions: Arc<dyn HarnessSessionStorePort>,
    pub events: Arc<dyn HarnessEventStorePort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
    pub workflows: Arc<dyn WorkflowStorePort>,
    pub tool_ledger: Arc<dyn ToolInvocationLedgerPort>,
    pub knowledge: Arc<dyn KnowledgeSourceStorePort>,
    pub memory: Arc<dyn MemoryStorePort>,
    pub approvals: Arc<dyn ApprovalStorePort>,
    pub clock: Arc<dyn ClockPort>,
    pub ids: Arc<dyn IdGeneratorPort>,
}
