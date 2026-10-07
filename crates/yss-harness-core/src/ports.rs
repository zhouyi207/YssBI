use std::sync::Arc;
use yss_harness_contract::{
    ApprovalStorePort, ClockPort, HarnessEventSinkPort, HarnessEventStorePort,
    HarnessSessionStorePort, IdGeneratorPort, KnowledgeSourceStorePort, LanguageModelResolverPort,
    ToolInvocationLedgerPort, WorkflowStorePort,
};

#[derive(Clone)]
pub struct HarnessPorts {
    pub models: Arc<dyn LanguageModelResolverPort>,
    pub resources: Arc<dyn yss_harness_contract::HarnessResourceResolverPort>,
    pub capability_gateway: Arc<dyn yss_harness_contract::CapabilityGatewayPort>,
    pub sessions: Arc<dyn HarnessSessionStorePort>,
    pub events: Arc<dyn HarnessEventStorePort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
    pub workflows: Arc<dyn WorkflowStorePort>,
    pub tool_ledger: Arc<dyn ToolInvocationLedgerPort>,
    pub knowledge: Arc<dyn KnowledgeSourceStorePort>,
    pub knowledge_index: Arc<dyn yss_harness_contract::KnowledgeIndexPort>,
    pub approvals: Arc<dyn ApprovalStorePort>,
    pub clock: Arc<dyn ClockPort>,
    pub ids: Arc<dyn IdGeneratorPort>,
}
