use std::collections::BTreeMap;
use std::sync::Mutex;

pub fn model_identity() -> yss_harness_contract::LanguageModelIdentity {
    yss_harness_contract::LanguageModelIdentity {
        selection: yss_harness_contract::LanguageModelSelection {
            provider_id: "test-provider".into(),
            model_id: "test-model".into(),
        },
        provider_name: "Test provider".into(),
        model_name: "Test model".into(),
    }
}

pub fn fixed_model(
    driver: std::sync::Arc<dyn yss_harness_contract::AgentDriverPort>,
) -> std::sync::Arc<dyn yss_harness_contract::LanguageModelResolverPort> {
    struct FixedModel(std::sync::Arc<dyn yss_harness_contract::AgentDriverPort>);
    impl yss_harness_contract::LanguageModelResolverPort for FixedModel {
        fn resolve<'a>(
            &'a self,
            _: Option<&'a yss_harness_contract::LanguageModelSelection>,
        ) -> yss_harness_contract::AgentFuture<
            'a,
            Result<
                yss_harness_contract::ResolvedLanguageModel,
                yss_harness_contract::AgentDriverFailure,
            >,
        > {
            Box::pin(async {
                Ok(yss_harness_contract::ResolvedLanguageModel {
                    identity: model_identity(),
                    driver: self.0.clone(),
                })
            })
        }
    }
    std::sync::Arc::new(FixedModel(driver))
}

pub struct FixtureResourceResolver;

impl yss_harness_contract::HarnessResourceResolverPort for FixtureResourceResolver {
    fn resolve<'a>(
        &'a self,
        _: &'a yss_harness_contract::ProjectSessionBinding,
        resources: &'a [yss_harness_contract::ProjectResourceRef],
        _: yss_harness_contract::CancellationToken,
    ) -> yss_harness_contract::AgentFuture<
        'a,
        Result<Vec<yss_harness_contract::HarnessResourceReference>, CapabilityFailure>,
    > {
        Box::pin(async {
            Ok(resources
                .iter()
                .map(|resource| yss_harness_contract::HarnessResourceReference {
                    name: resource.id.clone(),
                    resource: resource.clone(),
                })
                .collect())
        })
    }
}
use std::sync::atomic::{AtomicU64, Ordering};

use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, AgentEvent, AgentEventOutput,
    AgentTurnRequest, AgentTurnResult, ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort,
    AutomationIdKind, CapabilityFailure, CapabilityFailureCode, CapabilityFuture,
    CapabilityGatewayPort, CapabilityInvocationContext, ClockPort, HarnessEvent,
    HarnessEventEnvelope, HarnessEventSinkPort, HarnessEventStorePort, HarnessSessionId,
    HarnessSessionRecord, HarnessSessionStorePort, HarnessTurnId, HarnessTurnRecord,
    IdGeneratorPort, KnowledgeDocumentRecord, KnowledgeSourceId, KnowledgeSourceRecord,
    KnowledgeSourceStatus, KnowledgeSourceStorePort, ModelCapabilityExecutor, PersistenceFailure,
    PersistenceFailureCode, PersistenceFuture, ToolInvocationBegin, ToolInvocationLedgerPort,
    ToolInvocationRecord, UnixMillis, WorkflowDefinition, WorkflowId, WorkflowRunId,
    WorkflowRunRecord, WorkflowRunState, WorkflowStorePort, WorkflowVersion,
};

pub struct FixedClock {
    now: AtomicU64,
}

impl FixedClock {
    pub const fn new(now: u64) -> Self {
        Self {
            now: AtomicU64::new(now),
        }
    }

    pub fn advance(&self, milliseconds: u64) {
        self.now.fetch_add(milliseconds, Ordering::AcqRel);
    }
}

impl ClockPort for FixedClock {
    fn now(&self) -> UnixMillis {
        UnixMillis::from_existing(self.now.load(Ordering::Acquire))
    }
}

#[derive(Default)]
pub struct SequentialIds {
    next: AtomicU64,
}

impl IdGeneratorPort for SequentialIds {
    fn next_id(
        &self,
        kind: AutomationIdKind,
    ) -> Result<String, yss_harness_contract::IdGenerationFailure> {
        let prefix = match kind {
            AutomationIdKind::HarnessSession => "session",
            AutomationIdKind::HarnessTurn => "turn",
            AutomationIdKind::AgentRun => "agent",
            AutomationIdKind::WorkflowRun => "workflow",
            AutomationIdKind::ToolInvocation => "tool",
            AutomationIdKind::ApprovalGrant => "approval",
        };
        let value = self.next.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(format!("{prefix}-{value}"))
    }
}

pub struct MockAgentDriver {
    final_text: String,
}

impl MockAgentDriver {
    pub fn new(final_text: impl Into<String>) -> Self {
        Self {
            final_text: final_text.into(),
        }
    }
}

impl AgentDriverPort for MockAgentDriver {
    fn run_turn<'a>(
        &'a self,
        _request: AgentTurnRequest,
        _capabilities: std::sync::Arc<dyn ModelCapabilityExecutor>,
        output: std::sync::Arc<dyn AgentEventOutput>,
        cancellation: yss_harness_contract::CancellationToken,
    ) -> yss_harness_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
        Box::pin(async move {
            if cancellation.is_cancelled() {
                return Err(AgentDriverFailure::new(AgentDriverFailureCode::Cancelled));
            }
            output
                .emit(AgentEvent::TextDelta {
                    delta: self.final_text.clone(),
                })
                .await
                .map_err(|_| AgentDriverFailure::new(AgentDriverFailureCode::OutputUnavailable))?;
            Ok(AgentTurnResult {
                final_text: self.final_text.clone(),
            })
        })
    }
}

#[derive(Default)]
pub struct RejectingCapabilityGateway;

impl CapabilityGatewayPort for RejectingCapabilityGateway {
    fn invoke<'a>(
        &'a self,
        _context: CapabilityInvocationContext,
        _request: yss_harness_contract::AutomationCapabilityRequest,
        _control: yss_harness_contract::CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ))
        })
    }
}

pub struct StaticCapabilityGateway {
    results: Vec<yss_harness_contract::AutomationCapabilityResult>,
}

impl StaticCapabilityGateway {
    pub fn new(result: yss_harness_contract::AutomationCapabilityResult) -> Self {
        Self {
            results: vec![result],
        }
    }

    pub fn with_result(mut self, result: yss_harness_contract::AutomationCapabilityResult) -> Self {
        self.results.push(result);
        self
    }
}

impl CapabilityGatewayPort for StaticCapabilityGateway {
    fn invoke<'a>(
        &'a self,
        _context: CapabilityInvocationContext,
        request: yss_harness_contract::AutomationCapabilityRequest,
        _control: yss_harness_contract::CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            self.results
                .iter()
                .rev()
                .find(|result| result.accepts_capability(request.capability_id()))
                .cloned()
                .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))
        })
    }
}

#[derive(Default)]
pub struct InMemoryHarnessStore {
    state: Mutex<InMemoryState>,
}

#[derive(Default)]
struct InMemoryState {
    sessions: BTreeMap<HarnessSessionId, HarnessSessionRecord>,
    turns: BTreeMap<HarnessTurnId, HarnessTurnRecord>,
    events: BTreeMap<HarnessSessionId, Vec<HarnessEventEnvelope>>,
    published: Vec<HarnessEventEnvelope>,
    definitions: BTreeMap<(WorkflowId, WorkflowVersion), WorkflowDefinition>,
    runs: BTreeMap<WorkflowRunId, WorkflowRunRecord>,
    invocations: BTreeMap<yss_harness_contract::IdempotencyKey, ToolInvocationRecord>,
    knowledge_generation: u64,
    knowledge_sources: BTreeMap<KnowledgeSourceId, KnowledgeSourceRecord>,
    knowledge_documents:
        BTreeMap<yss_harness_contract::KnowledgeDocumentId, KnowledgeDocumentRecord>,
    approvals: BTreeMap<ApprovalGrantId, ApprovalGrantRecord>,
}

impl InMemoryHarnessStore {
    pub fn tool_invocations(&self) -> Vec<ToolInvocationRecord> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .invocations
            .values()
            .cloned()
            .collect()
    }

    pub fn published_events(&self) -> Vec<HarnessEventEnvelope> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .published
            .clone()
    }
}

impl HarnessSessionStorePort for InMemoryHarnessStore {
    fn list_conversations<'a>(
        &'a self,
        principal: &'a yss_harness_contract::PrincipalId,
        project_key: &'a str,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .sessions
                .values()
                .filter(|session| {
                    &session.principal_id == principal
                        && session
                            .conversation
                            .as_ref()
                            .is_some_and(|value| value.project_key == project_key)
                })
                .cloned()
                .collect())
        })
    }

    fn load_running_turns<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessTurnRecord>, PersistenceFailure>> {
        Box::pin(async {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .turns
                .values()
                .filter(|record| record.state == yss_harness_contract::HarnessTurnState::Running)
                .cloned()
                .collect())
        })
    }

    fn create_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            match state.sessions.entry(record.id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(record.clone());
                    Ok(())
                }
                std::collections::btree_map::Entry::Occupied(_) => Err(conflict()),
            }
        })
    }

    fn load_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessSessionRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .sessions
                .get(session_id)
                .cloned())
        })
    }

    fn load_active_sessions<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessSessionRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .sessions
                .values()
                .filter(|session| {
                    session.state == yss_harness_contract::HarnessSessionState::Active
                })
                .cloned()
                .collect())
        })
    }

    fn update_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if !state.sessions.contains_key(&record.id) {
                return Err(not_found());
            }
            state.sessions.insert(record.id.clone(), record.clone());
            Ok(())
        })
    }

    fn delete_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.sessions.remove(session_id).ok_or_else(not_found)?;
            state.turns.retain(|_, turn| &turn.session_id != session_id);
            state.events.remove(session_id);
            state
                .invocations
                .retain(|_, invocation| &invocation.session_id != session_id);
            state.runs.retain(|_, run| &run.session_id != session_id);
            state
                .approvals
                .retain(|_, grant| &grant.session_id != session_id);
            Ok(())
        })
    }

    fn create_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            match state.turns.entry(record.id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(record.clone());
                    Ok(())
                }
                std::collections::btree_map::Entry::Occupied(_) => Err(conflict()),
            }
        })
    }

    fn load_turn<'a>(
        &'a self,
        turn_id: &'a HarnessTurnId,
    ) -> PersistenceFuture<'a, Result<Option<HarnessTurnRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .turns
                .get(turn_id)
                .cloned())
        })
    }

    fn update_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if !state.turns.contains_key(&record.id) {
                return Err(not_found());
            }
            state.turns.insert(record.id.clone(), record.clone());
            Ok(())
        })
    }
}

impl HarnessEventStorePort for InMemoryHarnessStore {
    fn append_event<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        turn_id: Option<&'a HarnessTurnId>,
        occurred_at: UnixMillis,
        event: HarnessEvent,
    ) -> PersistenceFuture<'a, Result<HarnessEventEnvelope, PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let events = state.events.entry(session_id.clone()).or_default();
            let sequence = events
                .last()
                .map_or(0, |current| current.sequence)
                .checked_add(1)
                .ok_or_else(invalid_record)?;
            let envelope = HarnessEventEnvelope {
                sequence,
                session_id: session_id.clone(),
                turn_id: turn_id.cloned(),
                occurred_at,
                event,
            };
            events.push(envelope.clone());
            Ok(envelope)
        })
    }

    fn load_events_after<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        sequence: u64,
    ) -> PersistenceFuture<'a, Result<Vec<HarnessEventEnvelope>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .events
                .get(session_id)
                .into_iter()
                .flatten()
                .filter(|event| event.sequence > sequence)
                .cloned()
                .collect())
        })
    }

    fn latest_sequence<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<u64, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .events
                .get(session_id)
                .and_then(|events| events.last())
                .map_or(0, |event| event.sequence))
        })
    }
}

impl HarnessEventSinkPort for InMemoryHarnessStore {
    fn publish<'a>(
        &'a self,
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            self.state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .published
                .push(event.clone());
            Ok(())
        })
    }
}

impl WorkflowStorePort for InMemoryHarnessStore {
    fn has_unfinished_runs<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<bool, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .runs
                .values()
                .any(|run| {
                    &run.session_id == session_id
                        && matches!(
                            run.state,
                            WorkflowRunState::Planned
                                | WorkflowRunState::Ready
                                | WorkflowRunState::Running
                                | WorkflowRunState::Paused
                        )
                }))
        })
    }

    fn save_definition<'a>(
        &'a self,
        definition: &'a WorkflowDefinition,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let key = (definition.id.clone(), definition.version.clone());
            if state
                .definitions
                .get(&key)
                .is_some_and(|existing| existing != definition)
            {
                return Err(conflict());
            }
            state.definitions.insert(key, definition.clone());
            Ok(())
        })
    }

    fn load_definition<'a>(
        &'a self,
        id: &'a WorkflowId,
        version: &'a WorkflowVersion,
    ) -> PersistenceFuture<'a, Result<Option<WorkflowDefinition>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .definitions
                .get(&(id.clone(), version.clone()))
                .cloned())
        })
    }

    fn save_run<'a>(
        &'a self,
        run: &'a WorkflowRunRecord,
        expected_revision: Option<u64>,
    ) -> PersistenceFuture<'a, Result<WorkflowRunRecord, PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let mut committed = run.clone();
            match expected_revision {
                Some(expected)
                    if run.revision == expected
                        && state
                            .runs
                            .get(&run.id)
                            .is_some_and(|current| current.revision == expected) =>
                {
                    committed.revision = expected.checked_add(1).ok_or_else(invalid_record)?;
                }
                None if run.revision == 0 && !state.runs.contains_key(&run.id) => {}
                _ => return Err(conflict()),
            }
            state.runs.insert(run.id.clone(), committed.clone());
            Ok(committed)
        })
    }

    fn load_run<'a>(
        &'a self,
        id: &'a WorkflowRunId,
    ) -> PersistenceFuture<'a, Result<Option<WorkflowRunRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .runs
                .get(id)
                .cloned())
        })
    }

    fn load_recoverable_runs<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<WorkflowRunRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .runs
                .values()
                .filter(|run| {
                    matches!(
                        run.state,
                        WorkflowRunState::Running
                            | WorkflowRunState::Paused
                            | WorkflowRunState::Ready
                    )
                })
                .cloned()
                .collect())
        })
    }
}

impl ToolInvocationLedgerPort for InMemoryHarnessStore {
    fn load_invocation<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
        invocation_id: &'a yss_harness_contract::ToolInvocationId,
    ) -> PersistenceFuture<'a, Result<Option<ToolInvocationRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .invocations
                .values()
                .find(|record| &record.session_id == session_id && &record.id == invocation_id)
                .cloned())
        })
    }

    fn load_running_invocations<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<ToolInvocationRecord>, PersistenceFailure>> {
        Box::pin(async {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .invocations
                .values()
                .filter(|record| record.state == yss_harness_contract::ToolInvocationState::Running)
                .cloned()
                .collect())
        })
    }

    fn begin<'a>(
        &'a self,
        record: &'a ToolInvocationRecord,
    ) -> PersistenceFuture<'a, Result<ToolInvocationBegin, PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(existing) = state.invocations.get(&record.idempotency_key) {
                return Ok(ToolInvocationBegin::Existing(Box::new(existing.clone())));
            }
            state
                .invocations
                .insert(record.idempotency_key.clone(), record.clone());
            Ok(ToolInvocationBegin::Started)
        })
    }

    fn finish<'a>(
        &'a self,
        record: &'a ToolInvocationRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if !state.invocations.contains_key(&record.idempotency_key) {
                return Err(not_found());
            }
            state
                .invocations
                .insert(record.idempotency_key.clone(), record.clone());
            Ok(())
        })
    }
}

impl ApprovalStorePort for InMemoryHarnessStore {
    fn insert<'a>(
        &'a self,
        record: &'a ApprovalGrantRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            match state.approvals.entry(record.id.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(record.clone());
                    Ok(())
                }
                std::collections::btree_map::Entry::Occupied(_) => Err(conflict()),
            }
        })
    }

    fn load<'a>(
        &'a self,
        id: &'a ApprovalGrantId,
    ) -> PersistenceFuture<'a, Result<Option<ApprovalGrantRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .approvals
                .get(id)
                .cloned())
        })
    }

    fn consume<'a>(
        &'a self,
        id: &'a ApprovalGrantId,
        consumed_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<bool, PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let Some(record) = state.approvals.get_mut(id) else {
                return Err(not_found());
            };
            if record.consumed_at.is_some() {
                return Ok(false);
            }
            record.consumed_at = Some(consumed_at);
            Ok(true)
        })
    }
}

impl KnowledgeSourceStorePort for InMemoryHarnessStore {
    fn list_active_sources(
        &self,
    ) -> PersistenceFuture<'_, Result<Vec<KnowledgeSourceRecord>, PersistenceFailure>> {
        Box::pin(async move {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            Ok(state
                .knowledge_sources
                .values()
                .filter(|source| source.status == KnowledgeSourceStatus::Active)
                .cloned()
                .collect())
        })
    }

    fn replace_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
        documents: &'a [KnowledgeDocumentRecord],
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let mut ids = std::collections::BTreeSet::new();
            if documents
                .iter()
                .any(|document| document.source_id != source.id || !ids.insert(&document.id))
            {
                return Err(invalid_record());
            }
            if documents.iter().any(|document| {
                state
                    .knowledge_documents
                    .get(&document.id)
                    .is_some_and(|existing| existing.source_id != source.id)
            }) {
                return Err(conflict());
            }
            state
                .knowledge_sources
                .insert(source.id.clone(), source.clone());
            state
                .knowledge_documents
                .retain(|_, document| document.source_id != source.id);
            state.knowledge_documents.extend(
                documents
                    .iter()
                    .map(|document| (document.id.clone(), document.clone())),
            );
            state.knowledge_generation += 1;
            Ok(())
        })
    }

    fn load_active_snapshot<'a>(
        &'a self,
        known_generation: Option<u64>,
    ) -> PersistenceFuture<
        'a,
        Result<Option<yss_harness_contract::KnowledgeSourceSnapshot>, PersistenceFailure>,
    > {
        Box::pin(async move {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if known_generation == Some(state.knowledge_generation) {
                return Ok(None);
            }
            let documents = state
                .knowledge_documents
                .values()
                .filter_map(|document| {
                    state
                        .knowledge_sources
                        .get(&document.source_id)
                        .filter(|source| source.status == KnowledgeSourceStatus::Active)
                        .map(|source| (source.clone(), document.clone()))
                })
                .collect();
            Ok(Some(yss_harness_contract::KnowledgeSourceSnapshot {
                generation: state.knowledge_generation,
                documents,
            }))
        })
    }

    fn read_active_document<'a>(
        &'a self,
        document_id: &'a yss_harness_contract::KnowledgeDocumentId,
    ) -> PersistenceFuture<
        'a,
        Result<Option<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    > {
        Box::pin(async move {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            Ok(state
                .knowledge_documents
                .get(document_id)
                .and_then(|document| {
                    state
                        .knowledge_sources
                        .get(&document.source_id)
                        .filter(|source| source.status == KnowledgeSourceStatus::Active)
                        .map(|source| (source.clone(), document.clone()))
                }))
        })
    }

    fn mark_source_deleted<'a>(
        &'a self,
        source_id: &'a KnowledgeSourceId,
        updated_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let source = state
                .knowledge_sources
                .get_mut(source_id)
                .ok_or_else(not_found)?;
            source.status = KnowledgeSourceStatus::Deleted;
            source.updated_at = updated_at;
            state
                .knowledge_documents
                .retain(|_, document| &document.source_id != source_id);
            state.knowledge_generation += 1;
            Ok(())
        })
    }
}

fn conflict() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Conflict)
}

fn invalid_record() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::InvalidRecord)
}

fn not_found() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_harness_contract::{
        HarnessSessionState, HarnessTurnState, PrincipalId, ProjectSessionBinding, SourceHash,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[tokio::test]
    async fn duplicate_creation_preserves_original_session_turn_and_approval_records() {
        let store = InMemoryHarnessStore::default();
        let now = UnixMillis::from_existing(100);
        let session = HarnessSessionRecord {
            id: HarnessSessionId::try_new("session").unwrap(),
            principal_id: PrincipalId::try_new("principal").unwrap(),
            project: ProjectSessionBinding::new(
                ProjectInstanceId::new(),
                ProjectSessionId::new("project"),
            ),
            conversation: None,
            state: HarnessSessionState::Active,
            created_at: now,
            updated_at: now,
        };
        store.create_session(&session).await.unwrap();
        let mut conflicting_session = session.clone();
        conflicting_session.state = HarnessSessionState::Stale;
        assert_eq!(
            store
                .create_session(&conflicting_session)
                .await
                .unwrap_err()
                .code,
            PersistenceFailureCode::Conflict
        );
        assert_eq!(
            store.load_session(&session.id).await.unwrap(),
            Some(session.clone())
        );

        let turn = HarnessTurnRecord {
            id: HarnessTurnId::try_new("turn").unwrap(),
            session_id: session.id.clone(),
            state: HarnessTurnState::Running,
            user_message: "Original request".into(),
            final_text: None,
            started_at: now,
            finished_at: None,
        };
        store.create_turn(&turn).await.unwrap();
        let mut conflicting_turn = turn.clone();
        conflicting_turn.user_message = "Changed request".into();
        assert_eq!(
            store.create_turn(&conflicting_turn).await.unwrap_err().code,
            PersistenceFailureCode::Conflict
        );
        assert_eq!(store.load_turn(&turn.id).await.unwrap(), Some(turn));

        let approval = ApprovalGrantRecord {
            id: ApprovalGrantId::try_new("approval").unwrap(),
            principal_id: session.principal_id,
            session_id: session.id,
            project: session.project,
            capability_id: yss_harness_contract::CapabilityId::CreateNodes,
            request_fingerprint: SourceHash::try_new("0".repeat(64)).unwrap(),
            issued_at: now,
            expires_at: UnixMillis::from_existing(200),
            consumed_at: None,
        };
        store.insert(&approval).await.unwrap();
        let mut conflicting_approval = approval.clone();
        conflicting_approval.consumed_at = Some(now);
        assert_eq!(
            store.insert(&conflicting_approval).await.unwrap_err().code,
            PersistenceFailureCode::Conflict
        );
        assert_eq!(store.load(&approval.id).await.unwrap(), Some(approval));
    }
}
