use super::{
    RunId, SYSTEM_TRACE_CLOCK, SpanGuard, SpanKind, SpanOutcome, SpanSpec, TraceClock, TraceSink,
    TraceSpan,
};
use crate::node_system::document::GraphResourcePath;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex};

pub const DEFAULT_PROJECT_TRACE_CAPACITY: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceCapacityError;

impl fmt::Display for TraceCapacityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("trace capacity must be greater than zero")
    }
}

impl std::error::Error for TraceCapacityError {}

pub struct BoundedTraceSink {
    capacity: usize,
    clock: Arc<dyn TraceClock>,
    spans: Mutex<VecDeque<TraceSpan>>,
}

impl fmt::Debug for BoundedTraceSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedTraceSink")
            .field("capacity", &self.capacity)
            .field(
                "span_count",
                &self.spans.lock().map(|spans| spans.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl BoundedTraceSink {
    pub fn new(capacity: usize) -> Result<Self, TraceCapacityError> {
        Self::with_clock(capacity, Arc::new(SYSTEM_TRACE_CLOCK))
    }

    pub fn with_clock(
        capacity: usize,
        clock: Arc<dyn TraceClock>,
    ) -> Result<Self, TraceCapacityError> {
        if capacity == 0 {
            return Err(TraceCapacityError);
        }
        Ok(Self {
            capacity,
            clock,
            spans: Mutex::new(VecDeque::with_capacity(capacity)),
        })
    }

    pub fn spans(&self) -> Vec<TraceSpan> {
        complete_snapshot(
            self.spans
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .iter()
                .cloned()
                .collect(),
        )
    }

    pub fn spans_for_graph(&self, graph_path: &GraphResourcePath) -> Vec<TraceSpan> {
        complete_snapshot(
            self.spans()
                .into_iter()
                .filter(|span| span.correlation.graph_path == *graph_path)
                .collect(),
        )
    }

    pub fn spans_for_run(&self, run_id: RunId) -> Vec<TraceSpan> {
        complete_snapshot(
            self.spans()
                .into_iter()
                .filter(|span| span.run_id == Some(run_id))
                .collect(),
        )
    }
}

fn complete_snapshot(mut spans: Vec<TraceSpan>) -> Vec<TraceSpan> {
    spans.sort_by_key(|span| (span.started_at, span.span_id));
    let mut valid = vec![true; spans.len()];
    let mut indices = HashMap::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        if let Some(previous) = indices.insert(span.span_id, index) {
            valid[previous] = false;
            valid[index] = false;
        }
    }

    let parents = spans
        .iter()
        .enumerate()
        .map(|(index, span)| match span.parent_span_id {
            None => None,
            Some(parent_id) => match indices.get(&parent_id).copied() {
                Some(parent) if parent != index => Some(parent),
                _ => {
                    valid[index] = false;
                    None
                }
            },
        })
        .collect::<Vec<_>>();
    for (index, parent) in parents.iter().copied().enumerate() {
        if valid[index] && !compatible_parent(&spans[index], parent.map(|parent| &spans[parent])) {
            valid[index] = false;
        }
    }

    let mut colors = vec![0_u8; spans.len()];
    for start in 0..spans.len() {
        if !valid[start] || colors[start] != 0 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current.filter(|index| valid[*index] && colors[*index] == 0) {
            colors[index] = 1;
            path.push(index);
            current = parents[index];
        }
        if let Some(cycle_start) = current.filter(|index| colors[*index] == 1)
            && let Some(position) = path.iter().position(|index| *index == cycle_start)
        {
            for index in &path[position..] {
                valid[*index] = false;
            }
        }
        for index in path {
            colors[index] = 2;
        }
    }

    let mut children = vec![Vec::new(); spans.len()];
    for (child, parent) in parents.iter().copied().enumerate() {
        if let Some(parent) = parent {
            children[parent].push(child);
        }
    }
    let mut invalid = valid
        .iter()
        .enumerate()
        .filter_map(|(index, valid)| (!valid).then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(parent) = invalid.pop_front() {
        for child in children[parent].iter().copied() {
            if valid[child] {
                valid[child] = false;
                invalid.push_back(child);
            }
        }
    }

    spans
        .into_iter()
        .zip(valid)
        .filter_map(|(span, valid)| valid.then_some(span))
        .collect()
}

fn compatible_parent(span: &TraceSpan, parent: Option<&TraceSpan>) -> bool {
    if !has_valid_kind_semantics(span) {
        return false;
    }
    if parent.is_some_and(|parent| !same_lineage(span, parent)) {
        return false;
    }
    if is_runtime_kind(span.kind) && parent.is_some_and(|parent| !interval_contains(parent, span)) {
        return false;
    }
    match span.kind {
        SpanKind::Snapshot => parent.is_none(),
        SpanKind::Analysis | SpanKind::Lowering => {
            parent.is_some_and(|parent| parent.kind == SpanKind::Snapshot)
        }
        SpanKind::Run => parent.is_none_or(|parent| parent.kind == SpanKind::Run),
        SpanKind::ResourceAcquire
        | SpanKind::ResultPublication
        | SpanKind::Cleanup
        | SpanKind::OperationAttempt => parent.is_some_and(|parent| parent.kind == SpanKind::Run),
        SpanKind::AdapterIo => parent.is_some_and(|parent| {
            parent.kind == SpanKind::OperationAttempt
                && span.operation_id == parent.operation_id
                && span.activation_id == parent.activation_id
                && span.attempt_id == parent.attempt_id
        }),
    }
}

fn has_valid_kind_semantics(span: &TraceSpan) -> bool {
    if span.finished_at < span.started_at || span.run_id != span.correlation.run_id {
        return false;
    }
    match span.kind {
        SpanKind::Snapshot | SpanKind::Analysis | SpanKind::Lowering => {
            span.run_id.is_none()
                && has_no_operation_identity(span)
                && is_general_outcome(&span.outcome)
        }
        SpanKind::Run => {
            span.run_id.is_some()
                && has_no_operation_identity(span)
                && is_general_outcome(&span.outcome)
        }
        SpanKind::ResourceAcquire | SpanKind::ResultPublication => {
            span.run_id.is_some()
                && has_no_operation_identity(span)
                && is_phase_outcome(&span.outcome)
        }
        SpanKind::Cleanup => {
            span.run_id.is_some()
                && has_no_operation_identity(span)
                && matches!(
                    span.outcome,
                    SpanOutcome::NotReached
                        | SpanOutcome::Cleanup { .. }
                        | SpanOutcome::InternalAborted
                )
        }
        SpanKind::OperationAttempt => {
            span.run_id.is_some()
                && has_operation_identity(span)
                && (is_general_outcome(&span.outcome) || span.outcome == SpanOutcome::Retry)
        }
        SpanKind::AdapterIo => {
            span.run_id.is_some()
                && has_operation_identity(span)
                && is_general_outcome(&span.outcome)
        }
    }
}

fn has_operation_identity(span: &TraceSpan) -> bool {
    span.operation_id.is_some() && span.activation_id.is_some() && span.attempt_id.is_some()
}

fn is_general_outcome(outcome: &SpanOutcome) -> bool {
    matches!(
        outcome,
        SpanOutcome::Success
            | SpanOutcome::Error
            | SpanOutcome::Cancellation
            | SpanOutcome::Timeout
            | SpanOutcome::InternalAborted
    )
}

fn is_phase_outcome(outcome: &SpanOutcome) -> bool {
    is_general_outcome(outcome) || *outcome == SpanOutcome::NotReached
}

fn is_runtime_kind(kind: SpanKind) -> bool {
    !matches!(
        kind,
        SpanKind::Snapshot | SpanKind::Analysis | SpanKind::Lowering
    )
}

fn interval_contains(parent: &TraceSpan, child: &TraceSpan) -> bool {
    child.started_at >= parent.started_at && child.finished_at <= parent.finished_at
}

fn has_no_operation_identity(span: &TraceSpan) -> bool {
    span.operation_id.is_none() && span.activation_id.is_none() && span.attempt_id.is_none()
}

fn same_lineage(span: &TraceSpan, parent: &TraceSpan) -> bool {
    span.run_id == parent.run_id
        && span.correlation.project_session_id == parent.correlation.project_session_id
        && span.correlation.graph_path == parent.correlation.graph_path
        && span.correlation.graph_revision == parent.correlation.graph_revision
        && span.correlation.registry_fingerprint == parent.correlation.registry_fingerprint
        && span.correlation.compile_id == parent.correlation.compile_id
}

impl Default for BoundedTraceSink {
    fn default() -> Self {
        Self::new(DEFAULT_PROJECT_TRACE_CAPACITY).expect("default trace capacity is non-zero")
    }
}

impl TraceSink for BoundedTraceSink {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, self.clock.as_ref())
    }

    fn complete_span(&self, span: TraceSpan) {
        let mut spans = self.spans.lock().unwrap_or_else(|error| error.into_inner());
        if spans.len() == self.capacity {
            spans.pop_front();
        }
        spans.push_back(span);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, CompileProvenance, CorrelationContext, FakeTraceClock,
        MonotonicTimestamp, ProjectSessionId, RunId, SpanKind, SpanOutcome,
    };
    use crate::node_system::document::{GraphResourcePath, GraphRevision};
    use crate::node_system::plan::{AttemptId, OperationStableId};
    use crate::node_system::registry::RegistryFingerprint;
    use crate::node_system::runtime::ActivationId;
    use std::collections::BTreeMap;

    fn correlation(graph_path: &str, run_id: Option<u64>) -> CorrelationContext {
        let provenance = CompileProvenance {
            project_session_id: ProjectSessionId::new("project-session"),
            graph_path: GraphResourcePath(graph_path.into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                resource_versions: BTreeMap::new(),
                resource_observations: BTreeMap::new(),
            },
            compile_id: CompileId::new(1),
        };
        match run_id {
            Some(run_id) => CorrelationContext::compile(&provenance)
                .for_run(RunId::try_new(run_id).unwrap(), None),
            None => CorrelationContext::compile(&provenance),
        }
    }

    fn completed_span(
        span_id: u64,
        parent_span_id: Option<u64>,
        graph_path: &str,
        run_id: Option<u64>,
        kind: SpanKind,
    ) -> TraceSpan {
        let run_id = run_id.map(RunId::new);
        TraceSpan {
            span_id: crate::node_system::analysis::SpanId::new(span_id).unwrap(),
            parent_span_id: parent_span_id
                .map(|id| crate::node_system::analysis::SpanId::new(id).unwrap()),
            run_id,
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind,
            started_at: MonotonicTimestamp::new(1).unwrap(),
            finished_at: MonotonicTimestamp::new(1_000_000).unwrap(),
            outcome: SpanOutcome::Success,
            correlation: correlation(graph_path, run_id.map(RunId::get)),
        }
    }

    fn finish(
        sink: &BoundedTraceSink,
        graph_path: &str,
        run_id: Option<u64>,
        parent_span_id: Option<crate::node_system::analysis::SpanId>,
    ) -> crate::node_system::analysis::SpanId {
        let run_id = run_id.map(|id| RunId::try_new(id).unwrap());
        let mut guard = sink.start_span(SpanSpec {
            parent_span_id,
            run_id,
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: correlation(graph_path, run_id.map(RunId::get)),
        });
        let span_id = guard.span_id();
        guard.finish(SpanOutcome::Success);
        span_id
    }

    #[test]
    fn trace_span_store_rejects_zero_capacity() {
        assert!(BoundedTraceSink::new(0).is_err());
    }

    #[test]
    fn trace_span_store_retains_only_complete_spans() {
        let clock = Arc::new(FakeTraceClock::new(MonotonicTimestamp::new(1).unwrap()));
        let sink = BoundedTraceSink::with_clock(2, clock).unwrap();
        let parent = finish(&sink, "events/one", Some(1), None);
        finish(&sink, "events/one", Some(1), Some(parent));
        finish(&sink, "events/two", Some(2), None);

        assert_eq!(sink.spans().len(), 1);
        assert_eq!(
            sink.spans()[0].correlation.graph_path.0.as_ref(),
            "events/two"
        );
    }

    #[test]
    fn trace_span_store_removes_self_and_multi_span_cycles_with_descendants() {
        let sink = BoundedTraceSink::new(16).unwrap();
        for span in [
            completed_span(1, Some(1), "events/cycles", Some(7), SpanKind::Run),
            completed_span(2, Some(3), "events/cycles", Some(7), SpanKind::Run),
            completed_span(3, Some(2), "events/cycles", Some(7), SpanKind::Run),
            completed_span(4, Some(6), "events/cycles", Some(7), SpanKind::Run),
            completed_span(5, Some(4), "events/cycles", Some(7), SpanKind::Run),
            completed_span(6, Some(5), "events/cycles", Some(7), SpanKind::Run),
            completed_span(7, Some(4), "events/cycles", Some(7), SpanKind::Run),
            completed_span(8, None, "events/cycles", Some(7), SpanKind::Run),
        ] {
            sink.complete_span(span);
        }

        let spans = sink.spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].span_id.get(), 8);
    }

    #[test]
    fn trace_span_store_removes_incompatible_lineage_and_keeps_compiler_hierarchy() {
        let sink = BoundedTraceSink::new(10).unwrap();
        for span in [
            completed_span(10, None, "events/main", Some(10), SpanKind::Run),
            completed_span(11, Some(10), "events/main", Some(11), SpanKind::Run),
            completed_span(12, Some(10), "events/other", Some(10), SpanKind::Run),
            {
                let mut cross_project =
                    completed_span(13, Some(10), "events/main", Some(10), SpanKind::Run);
                cross_project.correlation.project_session_id =
                    ProjectSessionId::new("other-project-session");
                cross_project
            },
            completed_span(20, None, "events/compiler", None, SpanKind::Snapshot),
            completed_span(21, Some(20), "events/compiler", None, SpanKind::Analysis),
        ] {
            sink.complete_span(span);
        }

        let spans = sink.spans();
        assert_eq!(
            spans
                .iter()
                .map(|span| span.span_id.get())
                .collect::<Vec<_>>(),
            vec![10, 20, 21]
        );
    }

    #[test]
    fn trace_span_store_enforces_operation_and_adapter_parent_identity() {
        let sink = BoundedTraceSink::new(8).unwrap();
        let operation_id = OperationStableId::new("trace.operation").unwrap();
        let activation_id = ActivationId::next().unwrap();
        let attempt_id = AttemptId::initial();
        let run = completed_span(30, None, "events/operation", Some(30), SpanKind::Run);
        let mut operation = completed_span(
            31,
            Some(30),
            "events/operation",
            Some(30),
            SpanKind::OperationAttempt,
        );
        operation.operation_id = Some(operation_id.clone());
        operation.activation_id = Some(activation_id);
        operation.attempt_id = Some(attempt_id);
        let mut adapter = completed_span(
            32,
            Some(31),
            "events/operation",
            Some(30),
            SpanKind::AdapterIo,
        );
        adapter.operation_id = Some(operation_id.clone());
        adapter.activation_id = Some(activation_id);
        adapter.attempt_id = Some(attempt_id);
        let mut wrong_parent = adapter.clone();
        wrong_parent.span_id = crate::node_system::analysis::SpanId::new(33).unwrap();
        wrong_parent.parent_span_id = Some(run.span_id);
        let mut wrong_attempt = adapter.clone();
        wrong_attempt.span_id = crate::node_system::analysis::SpanId::new(34).unwrap();
        wrong_attempt.attempt_id = Some(AttemptId::new(2));
        for span in [run, operation, adapter, wrong_parent, wrong_attempt] {
            sink.complete_span(span);
        }

        assert_eq!(
            sink.spans()
                .iter()
                .map(|span| span.span_id.get())
                .collect::<Vec<_>>(),
            vec![30, 31, 32]
        );
    }

    #[test]
    fn trace_span_store_enforces_kind_outcome_run_and_runtime_interval_semantics() {
        let operation_id = OperationStableId::new("trace.semantic.operation").unwrap();
        let activation_id = ActivationId::next().unwrap();
        let attempt_id = AttemptId::initial();
        let run = completed_span(100, None, "events/semantic", Some(50), SpanKind::Run);
        let mut attempt = completed_span(
            101,
            Some(100),
            "events/semantic",
            Some(50),
            SpanKind::OperationAttempt,
        );
        attempt.operation_id = Some(operation_id.clone());
        attempt.activation_id = Some(activation_id);
        attempt.attempt_id = Some(attempt_id);
        attempt.outcome = SpanOutcome::Retry;
        attempt.started_at = MonotonicTimestamp::new(10).unwrap();
        attempt.finished_at = MonotonicTimestamp::new(900_000).unwrap();
        let mut adapter = completed_span(
            102,
            Some(101),
            "events/semantic",
            Some(50),
            SpanKind::AdapterIo,
        );
        adapter.operation_id = Some(operation_id);
        adapter.activation_id = Some(activation_id);
        adapter.attempt_id = Some(attempt_id);
        adapter.outcome = SpanOutcome::InternalAborted;
        adapter.started_at = MonotonicTimestamp::new(20).unwrap();
        adapter.finished_at = MonotonicTimestamp::new(800_000).unwrap();
        let mut cleanup = completed_span(
            103,
            Some(100),
            "events/semantic",
            Some(50),
            SpanKind::Cleanup,
        );
        cleanup.outcome = SpanOutcome::Cleanup {
            error_count: 0,
            panicking: false,
        };
        for span in [
            run.clone(),
            attempt.clone(),
            adapter.clone(),
            cleanup.clone(),
        ] {
            let sink = BoundedTraceSink::new(8).unwrap();
            sink.complete_span(run.clone());
            if span.span_id != run.span_id {
                if span.parent_span_id == Some(attempt.span_id) {
                    sink.complete_span(attempt.clone());
                }
                sink.complete_span(span.clone());
            }
            assert!(
                sink.spans()
                    .iter()
                    .any(|retained| retained.span_id == span.span_id)
            );
        }

        let malformed = [
            completed_span(110, None, "events/semantic", Some(50), SpanKind::Snapshot),
            completed_span(111, None, "events/semantic", None, SpanKind::Run),
            {
                let mut span = run.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(112).unwrap();
                span.outcome = SpanOutcome::NotReached;
                span
            },
            {
                let mut span = cleanup.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(113).unwrap();
                span.outcome = SpanOutcome::Success;
                span
            },
            {
                let mut span = cleanup.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(118).unwrap();
                span.outcome = SpanOutcome::Error;
                span
            },
            {
                let mut span = attempt.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(114).unwrap();
                span.outcome = SpanOutcome::Cleanup {
                    error_count: 0,
                    panicking: false,
                };
                span
            },
            {
                let mut span = adapter.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(115).unwrap();
                span.parent_span_id = Some(attempt.span_id);
                span.outcome = SpanOutcome::Retry;
                span
            },
            {
                let mut span = attempt.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(116).unwrap();
                span.started_at = MonotonicTimestamp::new(0).unwrap();
                span
            },
            {
                let mut span = adapter.clone();
                span.span_id = crate::node_system::analysis::SpanId::new(117).unwrap();
                span.parent_span_id = Some(attempt.span_id);
                span.finished_at = MonotonicTimestamp::new(950_000).unwrap();
                span
            },
        ];
        for span in malformed {
            let sink = BoundedTraceSink::new(8).unwrap();
            if span.parent_span_id.is_some() {
                sink.complete_span(run.clone());
                if span.parent_span_id == Some(attempt.span_id) {
                    sink.complete_span(attempt.clone());
                }
            }
            let invalid_id = span.span_id;
            sink.complete_span(span);
            assert!(
                sink.spans()
                    .iter()
                    .all(|retained| retained.span_id != invalid_id)
            );
        }

        let mut malformed_attempt = attempt.clone();
        malformed_attempt.span_id = crate::node_system::analysis::SpanId::new(120).unwrap();
        malformed_attempt.outcome = SpanOutcome::Cleanup {
            error_count: 0,
            panicking: false,
        };
        let mut descendant = adapter;
        descendant.span_id = crate::node_system::analysis::SpanId::new(121).unwrap();
        descendant.parent_span_id = Some(malformed_attempt.span_id);
        let sink = BoundedTraceSink::new(8).unwrap();
        for span in [run, malformed_attempt, descendant] {
            sink.complete_span(span);
        }
        assert_eq!(
            sink.spans()
                .iter()
                .map(|span| span.span_id.get())
                .collect::<Vec<_>>(),
            vec![100]
        );
    }

    #[test]
    fn trace_span_store_removes_every_reversed_interval_and_its_descendants() {
        let reversed = |mut span: TraceSpan| {
            span.started_at = MonotonicTimestamp::new(80).unwrap();
            span.finished_at = MonotonicTimestamp::new(20).unwrap();
            span
        };

        let sink = BoundedTraceSink::new(8).unwrap();
        let reversed_run = reversed(completed_span(
            200,
            None,
            "events/reversed-root",
            Some(60),
            SpanKind::Run,
        ));
        let mut cleanup = completed_span(
            201,
            Some(200),
            "events/reversed-root",
            Some(60),
            SpanKind::Cleanup,
        );
        cleanup.outcome = SpanOutcome::Cleanup {
            error_count: 0,
            panicking: false,
        };
        for span in [reversed_run, cleanup] {
            sink.complete_span(span);
        }
        assert!(sink.spans().is_empty());

        let sink = BoundedTraceSink::new(8).unwrap();
        let reversed_snapshot = reversed(completed_span(
            210,
            None,
            "events/reversed-compiler-root",
            None,
            SpanKind::Snapshot,
        ));
        let analysis = completed_span(
            211,
            Some(210),
            "events/reversed-compiler-root",
            None,
            SpanKind::Analysis,
        );
        for span in [reversed_snapshot, analysis] {
            sink.complete_span(span);
        }
        assert!(sink.spans().is_empty());

        let sink = BoundedTraceSink::new(8).unwrap();
        let snapshot = completed_span(
            220,
            None,
            "events/reversed-compiler-child",
            None,
            SpanKind::Snapshot,
        );
        let reversed_analysis = reversed(completed_span(
            221,
            Some(220),
            "events/reversed-compiler-child",
            None,
            SpanKind::Analysis,
        ));
        for span in [snapshot, reversed_analysis] {
            sink.complete_span(span);
        }
        assert_eq!(
            sink.spans()
                .iter()
                .map(|span| span.span_id.get())
                .collect::<Vec<_>>(),
            vec![220]
        );

        let sink = BoundedTraceSink::new(8).unwrap();
        let operation_id = OperationStableId::new("trace.reversed.operation").unwrap();
        let activation_id = ActivationId::next().unwrap();
        let attempt_id = AttemptId::initial();
        let run = completed_span(230, None, "events/reversed-child", Some(61), SpanKind::Run);
        let mut attempt = reversed(completed_span(
            231,
            Some(230),
            "events/reversed-child",
            Some(61),
            SpanKind::OperationAttempt,
        ));
        attempt.operation_id = Some(operation_id.clone());
        attempt.activation_id = Some(activation_id);
        attempt.attempt_id = Some(attempt_id);
        let mut adapter = completed_span(
            232,
            Some(231),
            "events/reversed-child",
            Some(61),
            SpanKind::AdapterIo,
        );
        adapter.operation_id = Some(operation_id);
        adapter.activation_id = Some(activation_id);
        adapter.attempt_id = Some(attempt_id);
        for span in [run, attempt, adapter] {
            sink.complete_span(span);
        }
        assert_eq!(
            sink.spans()
                .iter()
                .map(|span| span.span_id.get())
                .collect::<Vec<_>>(),
            vec![230]
        );
    }

    #[test]
    fn trace_span_store_retains_unexpected_cleanup_unwind_fallback() {
        let sink = BoundedTraceSink::new(8).unwrap();
        let run_id = RunId::new(70);
        let mut run = sink.start_span(SpanSpec {
            parent_span_id: None,
            run_id: Some(run_id),
            operation_id: None,
            activation_id: None,
            attempt_id: None,
            kind: SpanKind::Run,
            correlation: correlation("events/cleanup-drop", Some(70)),
        });
        let run_span_id = run.span_id();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _cleanup = sink.start_span(SpanSpec {
                parent_span_id: Some(run_span_id),
                run_id: Some(run_id),
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind: SpanKind::Cleanup,
                correlation: correlation("events/cleanup-drop", Some(70)),
            });
            panic!("unexpected cleanup unwind sentinel");
        }));
        assert!(unwind.is_err());
        run.finish(SpanOutcome::Success);

        let spans = sink.spans();
        assert_eq!(spans.len(), 2);
        assert!(spans.iter().any(|span| {
            span.kind == SpanKind::Cleanup && span.outcome == SpanOutcome::InternalAborted
        }));
    }

    #[test]
    fn trace_span_store_accepts_equal_endpoints() {
        let sink = BoundedTraceSink::new(4).unwrap();
        let mut equal_run =
            completed_span(240, None, "events/equal-runtime", Some(71), SpanKind::Run);
        equal_run.started_at = MonotonicTimestamp::new(50).unwrap();
        equal_run.finished_at = equal_run.started_at;
        let mut equal_snapshot =
            completed_span(241, None, "events/equal-compiler", None, SpanKind::Snapshot);
        equal_snapshot.started_at = MonotonicTimestamp::new(50).unwrap();
        equal_snapshot.finished_at = equal_snapshot.started_at;
        for span in [equal_run, equal_snapshot] {
            sink.complete_span(span);
        }
        assert_eq!(sink.spans().len(), 2);
    }

    #[test]
    fn trace_span_store_orders_by_monotonic_start_then_span_id() {
        let clock = Arc::new(FakeTraceClock::new(MonotonicTimestamp::new(5).unwrap()));
        let sink = BoundedTraceSink::with_clock(4, clock).unwrap();
        finish(&sink, "events/orders", Some(7), None);
        finish(&sink, "events/orders", Some(7), None);

        let spans = sink.spans_for_run(RunId::try_new(7).unwrap());
        assert_eq!(spans.len(), 2);
        assert!(spans[0].span_id < spans[1].span_id);
    }
}
