//! Session and turn entry points and their shared runtime state.

use crate::events::EventWriter;
use crate::skills::{STATISTICAL_REPORT_WRITING_ID, STATISTICAL_REPORT_WRITING_VERSION};
use crate::{HarnessError, HarnessPorts, SkillRegistry};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use yss_harness_contract::{
    AgentDriverFailureCode, CancellationToken, HarnessEventEnvelope, HarnessSessionId, SkillId,
    SkillPackage, SkillVersion, WorkflowRunId,
};

mod approval;
mod session;
mod turn;
mod workflow;

use workflow::WorkflowControl;

pub struct HarnessHost {
    ports: HarnessPorts,
    report_writing_skill: SkillPackage,
    active_turns: Arc<Mutex<BTreeMap<HarnessSessionId, CancellationToken>>>,
    event_publication: Arc<tokio::sync::Mutex<()>>,
    workflow_controls: Mutex<BTreeMap<WorkflowRunId, Weak<WorkflowControl>>>,
    agent_access: Arc<tokio::sync::RwLock<()>>,
}

impl HarnessHost {
    pub fn new(ports: HarnessPorts) -> Result<Self, HarnessError> {
        let report_writing_skill = SkillRegistry::with_builtins()
            .and_then(|registry| {
                registry
                    .resolve_exact(
                        &SkillId::try_new(STATISTICAL_REPORT_WRITING_ID)?,
                        &SkillVersion::try_new(STATISTICAL_REPORT_WRITING_VERSION)?,
                    )
                    .cloned()
            })
            .map_err(|_| HarnessError::Agent(AgentDriverFailureCode::InternalFailure))?;
        Ok(Self {
            ports,
            report_writing_skill,
            active_turns: Arc::new(Mutex::new(BTreeMap::new())),
            event_publication: Arc::new(tokio::sync::Mutex::new(())),
            workflow_controls: Mutex::new(BTreeMap::new()),
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
mod concurrency_tests;
#[cfg(test)]
mod tests;
