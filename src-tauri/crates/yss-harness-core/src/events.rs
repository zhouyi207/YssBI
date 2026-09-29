use crate::{HarnessError, MethodRegistry, StatisticalPlanner};
use std::sync::Arc;
use yss_harness_contract::{
    AgentEvent, AgentEventOutput, AgentOutputFailure, ClockPort, HarnessEvent,
    HarnessEventSinkPort, HarnessEventStorePort, HarnessSessionId, HarnessTurnId,
};

#[derive(Clone)]
pub(crate) struct EventWriter {
    store: Arc<dyn HarnessEventStorePort>,
    sink: Arc<dyn HarnessEventSinkPort>,
    clock: Arc<dyn ClockPort>,
    publication: Arc<tokio::sync::Mutex<()>>,
}

impl EventWriter {
    pub(crate) fn new(
        store: Arc<dyn HarnessEventStorePort>,
        sink: Arc<dyn HarnessEventSinkPort>,
        clock: Arc<dyn ClockPort>,
        publication: Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            store,
            sink,
            clock,
            publication,
        }
    }

    pub(crate) async fn append(
        &self,
        session_id: &HarnessSessionId,
        turn_id: Option<&HarnessTurnId>,
        event: HarnessEvent,
    ) -> Result<(), HarnessError> {
        // Keep this Host's live delivery ordered. Sequence allocation and commit
        // remain atomic in the store, including across independent writers.
        let _publication = self.publication.lock().await;
        let envelope = self
            .store
            .append_event(session_id, turn_id, self.clock.now(), event)
            .await?;
        self.sink.publish(&envelope).await?;
        Ok(())
    }
}

pub(crate) struct PersistingAgentOutput {
    pub writer: EventWriter,
    pub session_id: HarnessSessionId,
    pub turn_id: HarnessTurnId,
    pub run_id: Option<yss_harness_contract::AgentRunId>,
}

impl AgentEventOutput for PersistingAgentOutput {
    fn emit<'a>(
        &'a self,
        event: AgentEvent,
    ) -> yss_harness_contract::AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            if let AgentEvent::PlanProposed { plan } = &event {
                let methods = MethodRegistry::builtins().map_err(|_| AgentOutputFailure::Closed)?;
                StatisticalPlanner::validate(plan, &methods).map_err(|error| {
                    AgentOutputFailure::PolicyRejected {
                        reason: error.to_string(),
                        available_methods: methods.cards(),
                    }
                })?;
            }
            self.writer
                .append(
                    &self.session_id,
                    Some(&self.turn_id),
                    match &self.run_id {
                        Some(run_id) => HarnessEvent::AgentRunOutput {
                            run_id: run_id.clone(),
                            event,
                        },
                        None => HarnessEvent::Agent(event),
                    },
                )
                .await
                .map_err(|_| AgentOutputFailure::PersistenceFailed)
        })
    }
}
