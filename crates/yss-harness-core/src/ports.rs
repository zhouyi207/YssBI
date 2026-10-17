use std::sync::Arc;
use yss_harness_contract::{
    ApprovalStorePort, ClockPort, HarnessEventSinkPort, HarnessEventStorePort, HarnessExtension,
    HarnessResourceResolverPort, HarnessSessionStorePort, IdGeneratorPort, KnowledgeIndexPort,
    KnowledgeSourceStorePort, LanguageModelResolverPort, ToolInvocationLedgerPort,
    WorkflowStorePort,
};

#[derive(Clone)]
pub struct HarnessPorts {
    pub models: Arc<dyn LanguageModelResolverPort>,
    pub resources: Arc<dyn HarnessResourceResolverPort>,
    pub extension: Arc<dyn HarnessExtension>,
    pub sessions: Arc<dyn HarnessSessionStorePort>,
    pub events: Arc<dyn HarnessEventStorePort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
    pub workflows: Arc<dyn WorkflowStorePort>,
    pub tool_ledger: Arc<dyn ToolInvocationLedgerPort>,
    pub knowledge: Arc<dyn KnowledgeSourceStorePort>,
    pub knowledge_index: Arc<dyn KnowledgeIndexPort>,
    pub approvals: Arc<dyn ApprovalStorePort>,
    pub clock: Arc<dyn ClockPort>,
    pub ids: Arc<dyn IdGeneratorPort>,
}
