use super::{CompilationBasis, CompileId};
use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
use crate::node_system::protocol::NodeTypeId;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU64;

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::Mutex;
use std::time::Instant;

use crate::node_system::plan::{AttemptId, OperationStableId};
use crate::node_system::runtime::ActivationId;

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectSessionId(Box<str>);

impl ProjectSessionId {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn unknown() -> Self {
        Self::new("unknown")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

numeric_id!(ParentCallId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(NonZeroU64);

impl RunId {
    pub fn try_new(value: u64) -> Result<Self, InvalidTraceIdentity> {
        NonZeroU64::new(value).map(Self).ok_or(InvalidTraceIdentity)
    }

    pub fn new(value: u64) -> Self {
        Self::try_new(value).expect("run IDs must be non-zero")
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTraceIdentity;

impl std::fmt::Display for InvalidTraceIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("trace identities must be non-zero")
    }
}

impl std::error::Error for InvalidTraceIdentity {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProvenance {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrelationContext {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub registry_fingerprint: crate::node_system::registry::RegistryFingerprint,
    pub resource_versions: super::ResourceVersionSet,
    pub compile_id: CompileId,
    pub selection_digest: Option<Box<str>>,
    pub run_id: Option<RunId>,
    pub node_id: Option<NodeId>,
    pub node_type_id: Option<NodeTypeId>,
    pub parent_call: Option<ParentCallId>,
    pub trace_parent_span_id: Option<SpanId>,
}

impl CorrelationContext {
    pub fn compile(provenance: &CompileProvenance) -> Self {
        Self {
            project_session_id: provenance.project_session_id.clone(),
            graph_path: provenance.graph_path.clone(),
            graph_revision: provenance.basis.graph_revision,
            registry_fingerprint: provenance.basis.registry_fingerprint.clone(),
            resource_versions: provenance.basis.resource_versions.clone(),
            compile_id: provenance.compile_id,
            selection_digest: None,
            run_id: None,
            node_id: None,
            node_type_id: None,
            parent_call: None,
            trace_parent_span_id: None,
        }
    }

    pub fn for_run(mut self, run_id: RunId, parent_call: Option<ParentCallId>) -> Self {
        self.run_id = Some(run_id);
        self.parent_call = parent_call;
        self
    }

    pub fn with_trace_parent(mut self, span_id: SpanId) -> Self {
        self.trace_parent_span_id = Some(span_id);
        self
    }

    pub fn with_selection_digest(mut self, digest: [u8; 32]) -> Self {
        self.selection_digest = Some(
            digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                .into(),
        );
        self
    }

    pub fn for_node(mut self, node_id: NodeId, node_type_id: NodeTypeId) -> Self {
        self.node_id = Some(node_id);
        self.node_type_id = Some(node_type_id);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpanKind {
    Snapshot,
    Analysis,
    Lowering,
    Run,
    OperationAttempt,
    ResourceAcquire,
    AdapterIo,
    ResultPublication,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpanOutcome {
    Success,
    Error,
    Cancellation,
    Timeout,
    Retry,
    NotReached,
    Cleanup { error_count: u64, panicking: bool },
    InternalAborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TraceValue {
    Boolean(bool),
    Integer(i64),
    Text(Box<str>),
    Redacted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFieldSensitivity {
    Public,
    UserLiteral,
    ResourceSecret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveFieldAction {
    Redact,
    Omit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedactionPolicy {
    sensitive_fields: SensitiveFieldAction,
}

impl RedactionPolicy {
    pub const fn strict() -> Self {
        Self {
            sensitive_fields: SensitiveFieldAction::Redact,
        }
    }

    pub const fn omit_sensitive() -> Self {
        Self {
            sensitive_fields: SensitiveFieldAction::Omit,
        }
    }

    pub fn apply(
        self,
        sensitivity: TraceFieldSensitivity,
        value: TraceValue,
    ) -> Option<TraceValue> {
        match sensitivity {
            TraceFieldSensitivity::Public => Some(value),
            TraceFieldSensitivity::UserLiteral | TraceFieldSensitivity::ResourceSecret => {
                match self.sensitive_fields {
                    SensitiveFieldAction::Redact => Some(TraceValue::Redacted),
                    SensitiveFieldAction::Omit => None,
                }
            }
        }
    }
}

impl Default for RedactionPolicy {
    fn default() -> Self {
        Self::strict()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpanId(NonZeroU64);

impl SpanId {
    pub fn new(value: u64) -> Result<Self, InvalidTraceIdentity> {
        NonZeroU64::new(value).map(Self).ok_or(InvalidTraceIdentity)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    fn next() -> Option<Self> {
        static NEXT_SPAN_ID: AtomicU64 = AtomicU64::new(1);
        crate::node_system::allocate_nonzero_id(&NEXT_SPAN_ID)
            .ok()
            .map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MonotonicTimestamp(u64);

impl MonotonicTimestamp {
    pub const fn new(value: u64) -> Result<Self, InvalidTraceTimestamp> {
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTraceTimestamp;

pub trait TraceClock: Send + Sync {
    fn now(&self) -> MonotonicTimestamp;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTraceClock;

impl TraceClock for SystemTraceClock {
    fn now(&self) -> MonotonicTimestamp {
        static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);
        let nanos = ORIGIN.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        MonotonicTimestamp(nanos)
    }
}

pub static SYSTEM_TRACE_CLOCK: SystemTraceClock = SystemTraceClock;

#[cfg(test)]
#[derive(Debug)]
pub struct FakeTraceClock(Mutex<MonotonicTimestamp>);

#[cfg(test)]
impl FakeTraceClock {
    pub fn new(now: MonotonicTimestamp) -> Self {
        Self(Mutex::new(now))
    }

    pub fn set(&self, now: MonotonicTimestamp) {
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = now;
    }
}

#[cfg(test)]
impl TraceClock for FakeTraceClock {
    fn now(&self) -> MonotonicTimestamp {
        *self.0.lock().unwrap_or_else(|error| error.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSpan {
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub run_id: Option<RunId>,
    pub operation_id: Option<OperationStableId>,
    pub activation_id: Option<ActivationId>,
    pub attempt_id: Option<AttemptId>,
    pub kind: SpanKind,
    pub started_at: MonotonicTimestamp,
    pub finished_at: MonotonicTimestamp,
    pub outcome: SpanOutcome,
    pub correlation: CorrelationContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceProvenanceScope {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub registry_fingerprint: crate::node_system::registry::RegistryFingerprint,
    pub resource_versions: super::ResourceVersionSet,
    pub compile_id: CompileId,
}

impl From<&CorrelationContext> for TraceProvenanceScope {
    fn from(correlation: &CorrelationContext) -> Self {
        Self {
            project_session_id: correlation.project_session_id.clone(),
            graph_path: correlation.graph_path.clone(),
            graph_revision: correlation.graph_revision,
            registry_fingerprint: correlation.registry_fingerprint.clone(),
            resource_versions: correlation.resource_versions.clone(),
            compile_id: correlation.compile_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceBundleMetadata {
    pub provenance_scopes: Box<[TraceProvenanceScope]>,
    pub truncated: bool,
    pub dropped_span_count: u64,
    pub estimated_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTraceBundle {
    pub run_id: RunId,
    pub compile_id: CompileId,
    pub graph_path: GraphResourcePath,
    pub selection_digest: Option<Box<str>>,
    pub incident_id: Option<Box<str>>,
    pub metadata: TraceBundleMetadata,
    pub spans: Box<[TraceSpan]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationTraceBundle {
    pub compile_id: CompileId,
    pub graph_path: GraphResourcePath,
    pub metadata: TraceBundleMetadata,
    pub spans: Box<[TraceSpan]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceBundle {
    Compilation(CompilationTraceBundle),
    Run(RunTraceBundle),
}

impl TraceBundle {
    pub fn metadata(&self) -> &TraceBundleMetadata {
        match self {
            Self::Compilation(bundle) => &bundle.metadata,
            Self::Run(bundle) => &bundle.metadata,
        }
    }

    pub fn spans(&self) -> &[TraceSpan] {
        match self {
            Self::Compilation(bundle) => &bundle.spans,
            Self::Run(bundle) => &bundle.spans,
        }
    }

    pub fn is_associated_with_graph(&self, graph_path: &GraphResourcePath) -> bool {
        self.metadata()
            .provenance_scopes
            .iter()
            .any(|scope| scope.graph_path == *graph_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanSpec {
    pub parent_span_id: Option<SpanId>,
    pub run_id: Option<RunId>,
    pub operation_id: Option<OperationStableId>,
    pub activation_id: Option<ActivationId>,
    pub attempt_id: Option<AttemptId>,
    pub kind: SpanKind,
    pub correlation: CorrelationContext,
}

pub trait TraceSink: Send + Sync {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_>;
    fn complete_span(&self, span: TraceSpan);
}

pub fn start_span_safely<'a>(sink: &'a dyn TraceSink, spec: SpanSpec) -> SpanGuard<'a> {
    let fallback_spec = spec.clone();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink.start_span(spec)))
        .unwrap_or_else(|_| NOOP_TRACE_SINK.start_span(fallback_spec))
}

pub fn complete_span_safely(sink: &dyn TraceSink, span: TraceSpan) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink.complete_span(span)));
}

pub struct SpanGuard<'a> {
    sink: &'a dyn TraceSink,
    clock: &'a dyn TraceClock,
    pending: Option<(SpanId, SpanSpec, MonotonicTimestamp)>,
}

impl<'a> SpanGuard<'a> {
    pub fn new(sink: &'a dyn TraceSink, spec: SpanSpec, clock: &'a dyn TraceClock) -> Self {
        Self {
            sink,
            clock,
            pending: SpanId::next().map(|span_id| (span_id, spec, clock.now())),
        }
    }

    pub fn span_id(&self) -> Option<SpanId> {
        self.pending.as_ref().map(|pending| pending.0)
    }

    pub fn finish(&mut self, outcome: SpanOutcome) {
        self.complete(outcome);
    }

    pub fn replace_correlation(&mut self, correlation: CorrelationContext) {
        if let Some((_, spec, _)) = self.pending.as_mut() {
            spec.correlation = correlation;
        }
    }

    fn complete(&mut self, outcome: SpanOutcome) {
        let Some((span_id, spec, started_at)) = self.pending.take() else {
            return;
        };
        let finished_at = self.clock.now().max(started_at);
        let span = TraceSpan {
            span_id,
            parent_span_id: spec.parent_span_id,
            run_id: spec.run_id,
            operation_id: spec.operation_id,
            activation_id: spec.activation_id,
            attempt_id: spec.attempt_id,
            kind: spec.kind,
            started_at,
            finished_at,
            outcome,
            correlation: spec.correlation,
        };
        complete_span_safely(self.sink, span);
    }
}

impl Drop for SpanGuard<'_> {
    fn drop(&mut self) {
        self.complete(SpanOutcome::InternalAborted);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTraceSink;

impl TraceSink for NoopTraceSink {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, _: TraceSpan) {}
}

pub static NOOP_TRACE_SINK: NoopTraceSink = NoopTraceSink;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{ResourceKey, ResourceVersion};
    use crate::node_system::registry::RegistryFingerprint;

    fn provenance() -> CompileProvenance {
        CompileProvenance {
            project_session_id: ProjectSessionId::new("project-session-7"),
            graph_path: GraphResourcePath("events/main".into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(11),
                registry_fingerprint: RegistryFingerprint::from_bytes([3; 32]),
                resource_versions: BTreeMap::from([(
                    ResourceKey::new("functions/shared"),
                    ResourceVersion::new("9"),
                )]),
                resource_observations: BTreeMap::new(),
            },
            compile_id: CompileId::new(13),
        }
    }

    #[test]
    fn correlation_preserves_the_exact_compile_basis() {
        let provenance = provenance();
        let correlation = CorrelationContext::compile(&provenance)
            .for_run(RunId::new(17), Some(ParentCallId::new(19)));

        assert_eq!(
            correlation.project_session_id,
            provenance.project_session_id
        );
        assert_eq!(correlation.graph_path, provenance.graph_path);
        assert_eq!(correlation.graph_revision, provenance.basis.graph_revision);
        assert_eq!(
            correlation.registry_fingerprint,
            provenance.basis.registry_fingerprint
        );
        assert_eq!(
            correlation.resource_versions,
            provenance.basis.resource_versions
        );
        assert_eq!(correlation.compile_id, provenance.compile_id);
        assert_eq!(correlation.run_id, Some(RunId::new(17)));
        assert_eq!(correlation.parent_call, Some(ParentCallId::new(19)));
    }

    #[test]
    fn trace_span_identities_reject_zero() {
        assert!(SpanId::new(0).is_err());
        assert!(MonotonicTimestamp::new(0).is_ok());
        assert!(RunId::try_new(0).is_err());
        assert!(crate::node_system::plan::AttemptId::try_new(0).is_err());
    }

    #[test]
    fn trace_span_guard_finishes_exactly_once_with_fake_clock() {
        use std::sync::{Arc, Mutex};

        struct RecordingSink {
            clock: Arc<FakeTraceClock>,
            spans: Mutex<Vec<TraceSpan>>,
        }

        impl TraceSink for RecordingSink {
            fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
                SpanGuard::new(self, spec, self.clock.as_ref())
            }

            fn complete_span(&self, span: TraceSpan) {
                self.spans.lock().unwrap().push(span);
            }
        }

        let clock = Arc::new(FakeTraceClock::new(MonotonicTimestamp::new(10).unwrap()));
        let sink = RecordingSink {
            clock: Arc::clone(&clock),
            spans: Mutex::new(Vec::new()),
        };
        let parent = SpanId::new(7).unwrap();
        let run_id = RunId::try_new(11).unwrap();
        let mut guard = sink.start_span(SpanSpec {
            parent_span_id: Some(parent),
            run_id: Some(run_id),
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: CorrelationContext::compile(&provenance()).for_run(run_id, None),
        });
        clock.set(MonotonicTimestamp::new(25).unwrap());
        guard.finish(SpanOutcome::Success);
        guard.finish(SpanOutcome::Error);
        drop(guard);

        let spans = sink.spans.lock().unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].parent_span_id, Some(parent));
        assert_eq!(spans[0].run_id, Some(run_id));
        assert_eq!(spans[0].started_at.get(), 10);
        assert_eq!(spans[0].finished_at.get(), 25);
        assert_eq!(spans[0].outcome, SpanOutcome::Success);
    }

    #[test]
    fn dropped_trace_span_guard_emits_one_internal_aborted_span() {
        use std::sync::{Arc, Mutex};

        struct RecordingSink {
            clock: Arc<FakeTraceClock>,
            spans: Mutex<Vec<TraceSpan>>,
        }

        impl TraceSink for RecordingSink {
            fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
                SpanGuard::new(self, spec, self.clock.as_ref())
            }

            fn complete_span(&self, span: TraceSpan) {
                self.spans.lock().unwrap().push(span);
            }
        }

        let clock = Arc::new(FakeTraceClock::new(MonotonicTimestamp::new(3).unwrap()));
        let sink = RecordingSink {
            clock: Arc::clone(&clock),
            spans: Mutex::new(Vec::new()),
        };
        {
            let _guard = sink.start_span(SpanSpec {
                parent_span_id: None,
                run_id: None,
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind: SpanKind::Snapshot,
                correlation: CorrelationContext::compile(&provenance()),
            });
            clock.set(MonotonicTimestamp::new(4).unwrap());
        }

        let spans = sink.spans.lock().unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].outcome, SpanOutcome::InternalAborted);
        assert!(spans[0].finished_at >= spans[0].started_at);
    }

    #[test]
    fn strict_redaction_replaces_sensitive_trace_values() {
        let policy = RedactionPolicy::strict();
        assert_eq!(
            policy.apply(
                TraceFieldSensitivity::UserLiteral,
                TraceValue::Text("customer supplied text".into()),
            ),
            Some(TraceValue::Redacted)
        );
        assert_eq!(
            policy.apply(
                TraceFieldSensitivity::ResourceSecret,
                TraceValue::Text("database-password".into()),
            ),
            Some(TraceValue::Redacted)
        );
    }

    #[test]
    fn trace_sink_start_and_finish_panics_do_not_escape() {
        struct PanickingSink;
        impl TraceSink for PanickingSink {
            fn start_span(&self, _: SpanSpec) -> SpanGuard<'_> {
                panic!("start sink failed")
            }

            fn complete_span(&self, _: TraceSpan) {
                panic!("finish sink failed")
            }
        }

        let spec = SpanSpec {
            parent_span_id: None,
            run_id: None,
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Snapshot,
            correlation: CorrelationContext::compile(&provenance()),
        };
        let mut fallback = start_span_safely(&PanickingSink, spec.clone());
        fallback.finish(SpanOutcome::Success);

        let mut finish = SpanGuard::new(&PanickingSink, spec, &SYSTEM_TRACE_CLOCK);
        finish.finish(SpanOutcome::Success);
        drop(finish);
    }
}
