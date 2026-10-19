//! Durable tool execution: ledger identity, idempotency, deadlines, lifecycle events and recovery.

use std::collections::BTreeMap;
use std::future::{Future, poll_fn};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use serde_json::Value;
use yss_harness_contract::{
    AgentEvent, AgentEventOutput, AgentRunHooks, AgentRunId, AutomationIdKind, CancellationToken,
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, CapabilityInvocationId, ClockPort,
    HarnessExtension, HarnessMode, HarnessSessionId, HarnessTurnId, IdGeneratorPort,
    IdempotencyKey, PreparedToolCall, PrincipalId, ProjectSessionBinding, READ_KNOWLEDGE_TOOL,
    SEARCH_KNOWLEDGE_TOOL, ToolEffect, ToolExecution, ToolInvocationBegin, ToolInvocationContext,
    ToolInvocationId, ToolInvocationLedgerPort, ToolInvocationOutcome, ToolInvocationRecord,
    ToolInvocationState, ToolRecovery, ToolSpec, UnixMillis,
};

/// The tools one run may call, keyed by their model-facing names.
#[derive(Clone, Debug, Default)]
pub(crate) struct ToolRegistry {
    specs: BTreeMap<String, ToolSpec>,
}

impl ToolRegistry {
    pub(crate) fn new(specs: impl IntoIterator<Item = ToolSpec>) -> Self {
        Self {
            specs: specs
                .into_iter()
                .map(|spec| (spec.name.clone(), spec))
                .collect(),
        }
    }

    pub(crate) fn with_mode(mut self, mode: HarnessMode) -> Self {
        self.specs.retain(|_, spec| mode.allows(spec.effect));
        self
    }

    pub(crate) fn specs(&self) -> Vec<ToolSpec> {
        self.specs.values().cloned().collect()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&ToolSpec> {
        self.specs.get(name)
    }
}

pub(crate) fn is_knowledge_tool(name: &str) -> bool {
    matches!(name, SEARCH_KNOWLEDGE_TOOL | READ_KNOWLEDGE_TOOL)
}

/// Shared services behind every invocation of one Host.
#[derive(Clone)]
pub(crate) struct ToolServices {
    pub extension: Arc<dyn HarnessExtension>,
    pub knowledge: Arc<crate::KnowledgeService>,
    pub ledger: Arc<dyn ToolInvocationLedgerPort>,
    pub clock: Arc<dyn ClockPort>,
    pub ids: Arc<dyn IdGeneratorPort>,
}

/// The principal, conversation and project every invocation of an executor is bound to.
#[derive(Clone)]
pub(crate) struct InvocationScope {
    pub principal_id: PrincipalId,
    pub session_id: HarnessSessionId,
    pub turn_id: HarnessTurnId,
    pub project: ProjectSessionBinding,
}

pub(crate) struct HarnessToolExecutor {
    registry: ToolRegistry,
    services: ToolServices,
    scope: InvocationScope,
    cancellation: CancellationToken,
    output: Option<Arc<dyn AgentEventOutput>>,
    run: Option<(AgentRunId, Arc<dyn AgentRunHooks>)>,
}

impl HarnessToolExecutor {
    pub(crate) fn new(
        registry: ToolRegistry,
        services: ToolServices,
        scope: InvocationScope,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            registry,
            services,
            scope,
            cancellation,
            output: None,
            run: None,
        }
    }

    pub(crate) fn with_output(mut self, output: Arc<dyn AgentEventOutput>) -> Self {
        self.output = Some(output);
        self
    }

    pub(crate) fn with_run(mut self, run_id: AgentRunId, hooks: Arc<dyn AgentRunHooks>) -> Self {
        self.run = Some((run_id, hooks));
        self
    }

    pub(crate) fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub(crate) async fn begin_control(
        &self,
        tool: &str,
    ) -> Result<ToolInvocationId, CapabilityFailure> {
        let id = self.services.ids.next_id(AutomationIdKind::ToolInvocation);
        let invocation_id = ToolInvocationId::try_new(id).map_err(|_| persistence_unavailable())?;
        self.emit(AgentEvent::ControlToolStarted {
            invocation_id: invocation_id.clone(),
            tool: tool.to_owned(),
        })
        .await?;
        if self.cancellation.is_cancelled() {
            let cancelled = CapabilityFailure::new(CapabilityFailureCode::CANCELLED);
            self.finish_control(invocation_id, tool, Some(cancelled.clone()))
                .await?;
            return Err(cancelled);
        }
        Ok(invocation_id)
    }

    pub(crate) async fn finish_control(
        &self,
        invocation_id: ToolInvocationId,
        tool: &str,
        failure: Option<CapabilityFailure>,
    ) -> Result<(), CapabilityFailure> {
        self.emit(AgentEvent::ControlToolFinished {
            invocation_id,
            tool: tool.to_owned(),
            failure_code: failure.as_ref().map(|failure| failure.code.clone()),
            failure_details: failure.map(|failure| failure.details),
        })
        .await
    }

    pub(crate) fn new_call_key(&self) -> Result<String, CapabilityFailure> {
        let id = self.services.ids.next_id(AutomationIdKind::ToolInvocation);
        let digest = yss_canonical_hash::hash_canonical("yssbi.harness.call", &id)
            .map_err(|_| persistence_unavailable())?;
        Ok(hex::encode(digest))
    }

    async fn emit(&self, event: AgentEvent) -> Result<(), CapabilityFailure> {
        if let Some(output) = &self.output {
            output
                .emit(event)
                .await
                .map_err(|_| persistence_unavailable())?;
        }
        Ok(())
    }

    pub(crate) fn now(&self) -> UnixMillis {
        self.services.clock.now()
    }

    pub(crate) async fn execute(
        &self,
        call: PreparedToolCall,
        started_at: UnixMillis,
    ) -> Result<ToolInvocationOutcome, CapabilityFailure> {
        let tool = call.tool.clone();
        let arguments = call.arguments.clone();
        self.execute_invocation(tool, arguments, Ok(call), started_at)
            .await
    }

    pub(crate) async fn reject(
        &self,
        tool: String,
        arguments: Option<Value>,
        failure: CapabilityFailure,
        started_at: UnixMillis,
    ) -> Result<ToolInvocationOutcome, CapabilityFailure> {
        // Undecodable provider payloads are not retained.
        let arguments = arguments.unwrap_or_else(|| Value::Object(Default::default()));
        self.execute_invocation(tool, arguments, Err(failure), started_at)
            .await
    }

    async fn execute_invocation(
        &self,
        tool: String,
        arguments: Value,
        call: Result<PreparedToolCall, CapabilityFailure>,
        started_at: UnixMillis,
    ) -> Result<ToolInvocationOutcome, CapabilityFailure> {
        let spec = self.registry.get(&tool).cloned();
        let effect = spec
            .as_ref()
            .map_or(ToolEffect::Inspect, |spec| spec.effect);
        let timeout_ms = spec
            .as_ref()
            .map_or(effect.default_timeout_ms(), |spec| spec.timeout_ms);
        let validation = match &call {
            Err(failure) => Err(failure.clone()),
            Ok(_) if spec.is_none() => Err(CapabilityFailure::new(
                CapabilityFailureCode::INVALID_REQUEST,
            )
            .with_detail("capabilityId", &tool)),
            Ok(_) => Ok(()),
        };
        let run_id = self.run.as_ref().map(|(run_id, _)| run_id.clone());
        let raw_invocation_id = self.services.ids.next_id(AutomationIdKind::ToolInvocation);
        let invocation_id = ToolInvocationId::try_new(raw_invocation_id.clone())
            .map_err(|_| persistence_unavailable())?;
        let token = call
            .as_ref()
            .ok()
            .and_then(|call| call.idempotency_token.as_deref());
        let idempotency_key = if let Some(token) = token {
            let digest = yss_canonical_hash::hash_canonical(
                "yssbi.harness.tool.idempotency.v1",
                &(
                    &self.scope.session_id,
                    &self.scope.turn_id,
                    run_id.as_ref(),
                    &tool,
                    token,
                ),
            )
            .map_err(|_| persistence_unavailable())?;
            IdempotencyKey::try_new(hex::encode(digest)).map_err(|_| persistence_unavailable())?
        } else {
            IdempotencyKey::try_new(raw_invocation_id).map_err(|_| persistence_unavailable())?
        };
        let deadline = started_at
            .checked_add(timeout_ms)
            .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::DEADLINE_ELAPSED))?;
        let mut record = ToolInvocationRecord {
            id: invocation_id.clone(),
            idempotency_key,
            session_id: self.scope.session_id.clone(),
            turn_id: self.scope.turn_id.clone(),
            agent_run_id: run_id,
            project: self.scope.project.clone(),
            tool: tool.clone(),
            effect,
            arguments,
            request: call.as_ref().ok().map(|call| call.request.clone()),
            state: ToolInvocationState::Running,
            result: None,
            failure: None,
            started_at,
            deadline,
            finished_at: None,
        };
        match self
            .services
            .ledger
            .begin(&record)
            .await
            .map_err(|_| persistence_unavailable())?
        {
            ToolInvocationBegin::Started => {}
            ToolInvocationBegin::Existing(existing) => {
                if existing.request != record.request
                    || existing.tool != record.tool
                    || existing.project != self.scope.project
                    || existing.session_id != self.scope.session_id
                {
                    return Err(CapabilityFailure::new(
                        CapabilityFailureCode::INVOCATION_CONFLICT,
                    ));
                }
                return self.recover_existing(*existing).await;
            }
        }

        let context = self.invocation_context(&record.idempotency_key)?;
        let control = CapabilityControl::new(
            self.cancellation.clone(),
            Duration::from_millis(
                deadline
                    .get()
                    .saturating_sub(self.services.clock.now().get()),
            ),
        );
        let started = self
            .emit(AgentEvent::ToolInvocationStarted {
                invocation_id: invocation_id.clone(),
                tool: tool.clone(),
            })
            .await
            .and_then(|()| control.check());
        let mut citation = None;
        let mut notices = Vec::new();
        let mut outcome = match started {
            Err(error) => Err(error),
            Ok(()) => {
                // Catch adapter panics while polling so the authoritative ledger still closes.
                let invocation = async {
                    validation?;
                    let call = call.as_ref().map_err(Clone::clone)?;
                    if is_knowledge_tool(&tool) {
                        let (result, cited) = self
                            .services
                            .knowledge
                            .invoke(&tool, &call.request, &self.scope.project, &control)
                            .await?;
                        return Ok((ToolExecution::new(result), cited));
                    }
                    let execution = match &self.run {
                        Some((_, hooks)) => {
                            hooks.execute(context.clone(), call, control.clone()).await
                        }
                        None => {
                            self.services
                                .extension
                                .execute(context.clone(), &tool, &call.request, control.clone())
                                .await
                        }
                    }?;
                    Ok((execution, None))
                };
                contain_tool_failure(invocation)
                    .await
                    .map(|(execution, observed)| {
                        citation = observed;
                        notices = execution.notices;
                        execution.result
                    })
            }
        };
        if !outcome
            .as_ref()
            .is_err_and(|error| error.code == CapabilityFailureCode::OUTCOME_UNKNOWN)
            && (effect == ToolEffect::Inspect || outcome.is_err())
        {
            if let Err(failure) = control.check() {
                outcome = Err(failure);
            } else if self.services.clock.now() >= deadline {
                outcome = Err(CapabilityFailure::new(
                    CapabilityFailureCode::DEADLINE_ELAPSED,
                ));
            }
        }
        outcome = outcome.map_err(|failure| failure.with_detail("capabilityId", &tool));
        record.finished_at = Some(self.services.clock.now());
        match &outcome {
            Ok(result) => {
                record.state = ToolInvocationState::Succeeded;
                record.result = Some(result.clone());
            }
            Err(failure) => {
                record.state = ToolInvocationState::Failed;
                record.failure = Some(failure.clone());
            }
        }
        // Business commit is authoritative. A storage/display failure cannot undo it;
        // a retained Running record is repaired by querying the same operation identity.
        let committed = effect != ToolEffect::Inspect && outcome.is_ok();
        if self.services.ledger.finish(&record).await.is_err() && !committed {
            return Err(persistence_unavailable());
        }

        let delivery = self
            .emit(match &outcome {
                Ok(_) => AgentEvent::ToolInvocationCompleted {
                    invocation_id: invocation_id.clone(),
                    tool: tool.clone(),
                },
                Err(failure) => AgentEvent::ToolInvocationFailed {
                    invocation_id: invocation_id.clone(),
                    tool: tool.clone(),
                    failure_code: failure.code.clone(),
                    failure_details: Some(failure.details.clone()),
                },
            })
            .await;
        if !committed {
            delivery?;
        }
        if outcome.is_ok() {
            for notice in notices {
                self.emit(AgentEvent::Custom {
                    invocation_id: Some(invocation_id.clone()),
                    kind: notice.kind,
                    payload: notice.payload,
                })
                .await?;
            }
            if let Some(citation) = citation {
                self.emit(AgentEvent::KnowledgeCited { citation }).await?;
            }
        }
        outcome.map(|result| ToolInvocationOutcome {
            invocation_id,
            result,
        })
    }

    fn invocation_context(
        &self,
        key: &IdempotencyKey,
    ) -> Result<ToolInvocationContext, CapabilityFailure> {
        let mut context = ToolInvocationContext::new(
            self.scope.principal_id.clone(),
            self.scope.session_id.clone(),
            CapabilityInvocationId::try_new(key.as_str()).map_err(|_| persistence_unavailable())?,
            self.scope.project.clone(),
        );
        context.agent_run_id = self.run.as_ref().map(|(run_id, _)| run_id.clone());
        Ok(context)
    }

    async fn recover_existing(
        &self,
        mut record: ToolInvocationRecord,
    ) -> Result<ToolInvocationOutcome, CapabilityFailure> {
        if record.result.is_some()
            || record
                .failure
                .as_ref()
                .is_some_and(|failure| failure.code != CapabilityFailureCode::OUTCOME_UNKNOWN)
        {
            return replay_existing(record);
        }
        let Some(request) = record.request.clone() else {
            return replay_existing(record);
        };
        let context = self.invocation_context(&record.idempotency_key)?;
        match self
            .services
            .extension
            .recover(context, &record.tool, &request)
            .await?
        {
            ToolRecovery::Unsupported => replay_existing(record),
            ToolRecovery::Unknown => Err(CapabilityFailure::new(
                CapabilityFailureCode::OUTCOME_UNKNOWN,
            )),
            ToolRecovery::Committed(result) => {
                record.result = Some(result);
                record.failure = None;
                record.state = ToolInvocationState::Succeeded;
                record.finished_at = Some(self.services.clock.now());
                // The owner receipt is authoritative even if repairing the ledger row fails.
                if let Err(error) = self.services.ledger.finish(&record).await {
                    tracing::warn!(
                        domain = "Harness",
                        event = "tool_recovery_ledger_repair_failed",
                        code = %error.code,
                        "Harness could not repair a recovered tool invocation"
                    );
                }
                replay_existing(record)
            }
        }
    }
}

fn replay_existing(
    existing: ToolInvocationRecord,
) -> Result<ToolInvocationOutcome, CapabilityFailure> {
    if let Some(result) = existing.result {
        return Ok(ToolInvocationOutcome {
            invocation_id: existing.id,
            result,
        });
    }
    if let Some(failure) = existing.failure {
        return Err(failure);
    }
    Err(CapabilityFailure::new(
        CapabilityFailureCode::INVOCATION_CONFLICT,
    ))
}

fn persistence_unavailable() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::PERSISTENCE_UNAVAILABLE)
}

pub(crate) async fn contain_tool_failure<T>(
    future: impl Future<Output = Result<T, CapabilityFailure>>,
) -> Result<T, CapabilityFailure> {
    let mut future = std::pin::pin!(future);
    poll_fn(
        |context| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(context))) {
            Ok(result) => result,
            Err(_) => Poll::Ready(Err(CapabilityFailure::new(
                CapabilityFailureCode::INTERNAL_FAILURE,
            ))),
        },
    )
    .await
}

#[cfg(test)]
mod tests;
