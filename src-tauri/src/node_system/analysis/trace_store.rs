use super::{RunId, SYSTEM_TRACE_CLOCK, SpanGuard, SpanSpec, TraceClock, TraceSink, TraceSpan};
use crate::node_system::document::GraphResourcePath;
use std::collections::{BTreeSet, VecDeque};
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
    loop {
        let retained_ids = spans
            .iter()
            .map(|span| span.span_id)
            .collect::<BTreeSet<_>>();
        let previous_len = spans.len();
        spans.retain(|span| {
            span.parent_span_id
                .is_none_or(|parent_span_id| retained_ids.contains(&parent_span_id))
        });
        if spans.len() == previous_len {
            return spans;
        }
    }
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
    use crate::node_system::registry::RegistryFingerprint;
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
