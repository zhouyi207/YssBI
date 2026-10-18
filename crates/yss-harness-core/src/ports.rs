use std::sync::Arc;
use yss_harness_contract::{
    ClockPort, HarnessEventSinkPort, HarnessEventStorePort, HarnessExtension,
    HarnessResourceResolverPort, HarnessSessionStorePort, IdGeneratorPort, KnowledgeIndexPort,
    KnowledgeSourceStorePort, LanguageModelResolverPort, ToolInvocationLedgerPort,
};

#[derive(Clone)]
pub struct HarnessPorts {
    pub models: Arc<dyn LanguageModelResolverPort>,
    pub resources: Arc<dyn HarnessResourceResolverPort>,
    pub extension: Arc<dyn HarnessExtension>,
    pub sessions: Arc<dyn HarnessSessionStorePort>,
    pub events: Arc<dyn HarnessEventStorePort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
    pub tool_ledger: Arc<dyn ToolInvocationLedgerPort>,
    pub knowledge: Arc<dyn KnowledgeSourceStorePort>,
    pub knowledge_index: Arc<dyn KnowledgeIndexPort>,
    pub clock: Arc<dyn ClockPort>,
    pub ids: Arc<dyn IdGeneratorPort>,
}
