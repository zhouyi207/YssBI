//! Session and turn entry points and their shared runtime state.

use crate::events::EventWriter;
use crate::{HarnessError, HarnessPorts};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use yss_harness_contract::{CancellationToken, HarnessEventEnvelope, HarnessSessionId};

mod session;
mod turn;

pub use session::HarnessSessionAccess;

pub struct HarnessHost {
    ports: HarnessPorts,
    knowledge: Arc<crate::KnowledgeService>,
    active_turns: Arc<Mutex<BTreeMap<HarnessSessionId, CancellationToken>>>,
    session_access: tokio::sync::Mutex<()>,
    event_publication: Arc<tokio::sync::Mutex<()>>,
    agent_access: Arc<tokio::sync::RwLock<()>>,
}

impl HarnessHost {
    pub async fn inspect_tool_invocation(
        &self,
        session_id: &HarnessSessionId,
        invocation_id: &yss_harness_contract::ToolInvocationId,
    ) -> Result<Option<yss_harness_contract::ToolInvocationRecord>, HarnessError> {
        Ok(self
            .ports
            .tool_ledger
            .load_invocation(session_id, invocation_id)
            .await?)
    }

    pub async fn inspect_citation(
        &self,
        session_id: &HarnessSessionId,
        citation: &yss_harness_contract::KnowledgeCitation,
    ) -> Result<Option<String>, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        Ok(self.knowledge.inspect(citation, &session.project).await?)
    }

    pub fn new(ports: HarnessPorts) -> Result<Self, HarnessError> {
        Ok(Self {
            knowledge: Arc::new(crate::KnowledgeService::new(
                ports.knowledge.clone(),
                ports.knowledge_index.clone(),
            )),
            ports,
            active_turns: Arc::new(Mutex::new(BTreeMap::new())),
            session_access: tokio::sync::Mutex::new(()),
            event_publication: Arc::new(tokio::sync::Mutex::new(())),
            agent_access: Arc::new(tokio::sync::RwLock::new(())),
        })
    }

    fn event_writer(&self) -> EventWriter {
        EventWriter::new(
            Arc::clone(&self.ports.events),
            Arc::clone(&self.ports.event_sink),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.event_publication),
        )
    }

    pub async fn events_after(
        &self,
        session_id: &HarnessSessionId,
        sequence: u64,
    ) -> Result<Vec<HarnessEventEnvelope>, HarnessError> {
        Ok(self
            .ports
            .events
            .load_events_after(session_id, sequence)
            .await?)
    }
}

#[cfg(test)]
mod knowledge_tests;
#[cfg(test)]
mod preparation_tests;
#[cfg(test)]
mod tests;
