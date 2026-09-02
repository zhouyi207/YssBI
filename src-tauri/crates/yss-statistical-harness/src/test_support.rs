use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use yss_automation_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, AgentEvent, AgentEventOutput,
    AgentTurnRequest, AgentTurnResult, ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort,
    AutomationIdKind, CapabilityFailure, CapabilityFailureCode, CapabilityFuture,
    CapabilityGatewayPort, CapabilityInvocationContext, ClockPort, HarnessEventEnvelope,
    HarnessEventSinkPort, HarnessEventStorePort, HarnessSessionId, HarnessSessionRecord,
    HarnessSessionStorePort, HarnessTurnId, HarnessTurnRecord, IdGeneratorPort,
    KnowledgeDocumentRecord, KnowledgeSourceId, KnowledgeSourceRecord, KnowledgeSourceStatus,
    KnowledgeSourceStorePort, MemoryRecord, MemoryRecordId, MemoryStatus, MemoryStorePort,
    ModelCapabilityExecutor, PersistenceFailure, PersistenceFailureCode, PersistenceFuture,
    SkillPackage, SkillSourcePort, ToolInvocationBegin, ToolInvocationLedgerPort,
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
    ) -> Result<String, yss_automation_contract::IdGenerationFailure> {
        let prefix = match kind {
            AutomationIdKind::HarnessSession => "session",
            AutomationIdKind::HarnessTurn => "turn",
            AutomationIdKind::WorkflowRun => "workflow",
            AutomationIdKind::ToolInvocation => "tool",
            AutomationIdKind::CapabilityInvocation => "capability",
            AutomationIdKind::MemoryRecord => "memory",
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
        cancellation: yss_automation_contract::CancellationToken,
    ) -> yss_automation_contract::AgentFuture<'a, Result<AgentTurnResult, AgentDriverFailure>> {
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
        _request: yss_automation_contract::AutomationCapabilityRequest,
    ) -> CapabilityFuture<'a> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                CapabilityFailureCode::InternalFailure,
            ))
        })
    }
}

pub struct StaticCapabilityGateway {
    results: BTreeMap<
        yss_automation_contract::CapabilityId,
        yss_automation_contract::AutomationCapabilityResult,
    >,
}

impl StaticCapabilityGateway {
    pub fn new(result: yss_automation_contract::AutomationCapabilityResult) -> Self {
        Self {
            results: BTreeMap::from([(result.capability_id(), result)]),
        }
    }

    pub fn with_result(
        mut self,
        result: yss_automation_contract::AutomationCapabilityResult,
    ) -> Self {
        self.results.insert(result.capability_id(), result);
        self
    }
}

impl CapabilityGatewayPort for StaticCapabilityGateway {
    fn invoke<'a>(
        &'a self,
        _context: CapabilityInvocationContext,
        request: yss_automation_contract::AutomationCapabilityRequest,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            self.results
                .get(&request.capability_id())
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
    invocations: BTreeMap<yss_automation_contract::IdempotencyKey, ToolInvocationRecord>,
    memories: BTreeMap<MemoryRecordId, MemoryRecord>,
    knowledge_sources: BTreeMap<KnowledgeSourceId, KnowledgeSourceRecord>,
    knowledge_documents:
        BTreeMap<yss_automation_contract::KnowledgeDocumentId, KnowledgeDocumentRecord>,
    skills: BTreeMap<
        (
            yss_automation_contract::SkillId,
            yss_automation_contract::SkillVersion,
        ),
        SkillPackage,
    >,
    approvals: BTreeMap<ApprovalGrantId, ApprovalGrantRecord>,
}

impl InMemoryHarnessStore {
    pub fn published_events(&self) -> Vec<HarnessEventEnvelope> {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .published
            .clone()
    }
}

impl HarnessSessionStorePort for InMemoryHarnessStore {
    fn create_session<'a>(
        &'a self,
        record: &'a HarnessSessionRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state
                .sessions
                .insert(record.id.clone(), record.clone())
                .is_some()
            {
                return Err(conflict());
            }
            Ok(())
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

    fn load_open_sessions<'a>(
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
                    matches!(
                        session.state,
                        yss_automation_contract::HarnessSessionState::Active
                            | yss_automation_contract::HarnessSessionState::Closing
                    )
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

    fn create_turn<'a>(
        &'a self,
        record: &'a HarnessTurnRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state
                .turns
                .insert(record.id.clone(), record.clone())
                .is_some()
            {
                return Err(conflict());
            }
            Ok(())
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
        event: &'a HarnessEventEnvelope,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let events = state.events.entry(event.session_id.clone()).or_default();
            let expected = events.last().map_or(1, |current| current.sequence + 1);
            if event.sequence != expected {
                return Err(conflict());
            }
            events.push(event.clone());
            Ok(())
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
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            self.state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .runs
                .insert(run.id.clone(), run.clone());
            Ok(())
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
            if state
                .approvals
                .insert(record.id.clone(), record.clone())
                .is_some()
            {
                return Err(conflict());
            }
            Ok(())
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

impl MemoryStorePort for InMemoryHarnessStore {
    fn insert<'a>(
        &'a self,
        record: &'a MemoryRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state
                .memories
                .insert(record.id.clone(), record.clone())
                .is_some()
            {
                return Err(conflict());
            }
            Ok(())
        })
    }

    fn load<'a>(
        &'a self,
        id: &'a MemoryRecordId,
    ) -> PersistenceFuture<'a, Result<Option<MemoryRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .memories
                .get(id)
                .cloned())
        })
    }

    fn update<'a>(
        &'a self,
        record: &'a MemoryRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if !state.memories.contains_key(&record.id) {
                return Err(not_found());
            }
            state.memories.insert(record.id.clone(), record.clone());
            Ok(())
        })
    }

    fn activate<'a>(
        &'a self,
        record: &'a MemoryRecord,
        superseded: Option<&'a MemoryRecord>,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state
                .memories
                .get(&record.id)
                .is_none_or(|current| current.status != MemoryStatus::Proposed)
            {
                return Err(conflict());
            }
            if let Some(previous) = superseded {
                if state
                    .memories
                    .get(&previous.id)
                    .is_none_or(|current| current.status != MemoryStatus::Active)
                {
                    return Err(conflict());
                }
                state.memories.insert(previous.id.clone(), previous.clone());
            }
            state.memories.insert(record.id.clone(), record.clone());
            Ok(())
        })
    }

    fn query_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Vec<MemoryRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .memories
                .values()
                .filter(|record| &record.session_id == session_id)
                .cloned()
                .collect())
        })
    }

    fn list_active<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<MemoryRecord>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .memories
                .values()
                .filter(|record| record.status == MemoryStatus::Active)
                .cloned()
                .collect())
        })
    }
}

impl KnowledgeSourceStorePort for InMemoryHarnessStore {
    fn upsert_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            self.state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .knowledge_sources
                .insert(source.id.clone(), source.clone());
            Ok(())
        })
    }

    fn upsert_document<'a>(
        &'a self,
        document: &'a KnowledgeDocumentRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if !state.knowledge_sources.contains_key(&document.source_id) {
                return Err(not_found());
            }
            state
                .knowledge_documents
                .insert(document.id.clone(), document.clone());
            Ok(())
        })
    }

    fn list_active_documents<'a>(
        &'a self,
    ) -> PersistenceFuture<
        'a,
        Result<Vec<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    > {
        Box::pin(async move {
            let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            Ok(state
                .knowledge_documents
                .values()
                .filter_map(|document| {
                    state
                        .knowledge_sources
                        .get(&document.source_id)
                        .filter(|source| source.status == KnowledgeSourceStatus::Active)
                        .map(|source| (source.clone(), document.clone()))
                })
                .collect())
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
            Ok(())
        })
    }
}

impl SkillSourcePort for InMemoryHarnessStore {
    fn install_package<'a>(
        &'a self,
        package: &'a SkillPackage,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>> {
        Box::pin(async move {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let key = (
                package.manifest.id.clone(),
                package.manifest.version.clone(),
            );
            if state
                .skills
                .get(&key)
                .is_some_and(|existing| existing != package)
            {
                return Err(conflict());
            }
            state.skills.insert(key, package.clone());
            Ok(())
        })
    }

    fn list_packages<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<SkillPackage>, PersistenceFailure>> {
        Box::pin(async move {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .skills
                .values()
                .cloned()
                .collect())
        })
    }
}

fn conflict() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::Conflict)
}

fn not_found() -> PersistenceFailure {
    PersistenceFailure::new(PersistenceFailureCode::NotFound)
}
