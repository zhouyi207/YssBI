use super::{RunId, SpanEvent, TraceSink};
use crate::node_system::document::GraphResourcePath;
use std::collections::VecDeque;
use std::fmt;
use std::sync::Mutex;

pub const DEFAULT_PROJECT_TRACE_CAPACITY: usize = 4096;
const EXHAUSTED_TRACE_SEQUENCE: u64 = u64::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRecord {
    pub sequence: u64,
    pub event: SpanEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceCapacityError;

impl fmt::Display for TraceCapacityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("trace capacity must be greater than zero")
    }
}

impl std::error::Error for TraceCapacityError {}

#[derive(Debug)]
struct TraceBuffer {
    next_sequence: u64,
    records: VecDeque<TraceRecord>,
}

#[derive(Debug)]
pub struct BoundedTraceSink {
    capacity: usize,
    buffer: Mutex<TraceBuffer>,
}

impl BoundedTraceSink {
    pub fn new(capacity: usize) -> Result<Self, TraceCapacityError> {
        if capacity == 0 {
            return Err(TraceCapacityError);
        }
        Ok(Self {
            capacity,
            buffer: Mutex::new(TraceBuffer {
                next_sequence: 0,
                records: VecDeque::with_capacity(capacity),
            }),
        })
    }

    pub fn records(&self) -> Vec<TraceRecord> {
        self.buffer
            .lock()
            .expect("trace buffer lock poisoned")
            .records
            .iter()
            .cloned()
            .collect()
    }

    pub fn records_for_graph(&self, graph_path: &GraphResourcePath) -> Vec<TraceRecord> {
        self.records()
            .into_iter()
            .filter(|record| record.event.correlation.graph_path == *graph_path)
            .collect()
    }

    pub fn records_for_run(&self, run_id: RunId) -> Vec<TraceRecord> {
        self.records()
            .into_iter()
            .filter(|record| record.event.correlation.run_id == Some(run_id))
            .collect()
    }
}

impl Default for BoundedTraceSink {
    fn default() -> Self {
        Self::new(DEFAULT_PROJECT_TRACE_CAPACITY).expect("default trace capacity is non-zero")
    }
}

impl TraceSink for BoundedTraceSink {
    fn record(&self, event: SpanEvent) {
        let mut buffer = self.buffer.lock().expect("trace buffer lock poisoned");
        if buffer.next_sequence == EXHAUSTED_TRACE_SEQUENCE {
            return;
        }
        let sequence = buffer.next_sequence;
        buffer.next_sequence += 1;
        if buffer.records.len() == self.capacity {
            buffer.records.pop_front();
        }
        buffer.records.push_back(TraceRecord { sequence, event });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, CompileProvenance, CorrelationContext, ProjectSessionId,
        RunId, SpanEvent, SpanKind, SpanStatus, TraceSink,
    };
    use crate::node_system::document::{GraphResourcePath, GraphRevision};
    use crate::node_system::registry::RegistryFingerprint;
    use std::collections::BTreeMap;

    fn event(graph_path: &str, run_id: Option<u64>) -> SpanEvent {
        let provenance = CompileProvenance {
            project_session_id: ProjectSessionId::new("project-session"),
            graph_path: GraphResourcePath(graph_path.into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                resource_versions: BTreeMap::new(),
            },
            compile_id: CompileId::new(1),
        };
        let correlation = match run_id {
            Some(run_id) => {
                CorrelationContext::compile(&provenance).for_run(RunId::new(run_id), None)
            }
            None => CorrelationContext::compile(&provenance),
        };
        SpanEvent::new(SpanKind::Run, SpanStatus::Started, correlation)
    }

    #[test]
    fn bounded_trace_sink_rejects_zero_capacity() {
        assert!(BoundedTraceSink::new(0).is_err());
    }

    #[test]
    fn bounded_trace_sink_never_retains_more_than_capacity() {
        let sink = BoundedTraceSink::new(2).unwrap();

        sink.record(event("events/one", Some(1)));
        sink.record(event("events/two", Some(2)));
        sink.record(event("events/three", Some(3)));

        assert_eq!(sink.records().len(), 2);
    }

    #[test]
    fn bounded_trace_sink_evicts_the_oldest_record_first() {
        let sink = BoundedTraceSink::new(2).unwrap();

        sink.record(event("events/oldest", Some(1)));
        sink.record(event("events/middle", Some(2)));
        sink.record(event("events/newest", Some(3)));

        let paths = sink
            .records()
            .into_iter()
            .map(|record| record.event.correlation.graph_path.0)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                Box::<str>::from("events/middle"),
                Box::<str>::from("events/newest")
            ]
        );
    }

    #[test]
    fn bounded_trace_sink_assigns_monotonic_sequences_across_eviction() {
        let sink = BoundedTraceSink::new(2).unwrap();

        sink.record(event("events/one", Some(1)));
        sink.record(event("events/two", Some(2)));
        sink.record(event("events/three", Some(3)));

        let sequences = sink
            .records()
            .into_iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>();
        assert_eq!(sequences, vec![1, 2]);
    }

    #[test]
    fn bounded_trace_sink_drops_new_records_after_sequence_exhaustion() {
        let retained_event = event("events/retained", Some(7));
        let sink = BoundedTraceSink {
            capacity: 3,
            buffer: Mutex::new(TraceBuffer {
                next_sequence: u64::MAX - 1,
                records: VecDeque::from([TraceRecord {
                    sequence: u64::MAX - 2,
                    event: retained_event,
                }]),
            }),
        };

        sink.record(event("events/retained", Some(7)));
        sink.record(event("events/dropped", Some(8)));

        assert_eq!(
            sink.records()
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![u64::MAX - 2, u64::MAX - 1]
        );
        assert_eq!(
            sink.records_for_graph(&GraphResourcePath("events/retained".into()))
                .len(),
            2
        );
        assert_eq!(sink.records_for_run(RunId::new(7)).len(), 2);
        assert!(
            sink.records()
                .iter()
                .all(|record| record.event.correlation.graph_path.0.as_ref() != "events/dropped")
        );
    }

    #[test]
    fn bounded_trace_sink_filters_by_exact_graph_path_oldest_first() {
        let sink = BoundedTraceSink::new(4).unwrap();
        sink.record(event("events/orders", Some(1)));
        sink.record(event("events/orders-archive", Some(2)));
        sink.record(event("events/orders", Some(3)));

        let records = sink.records_for_graph(&GraphResourcePath("events/orders".into()));

        assert_eq!(
            records
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert!(records.iter().all(|record| {
            record.event.correlation.graph_path == GraphResourcePath("events/orders".into())
        }));
    }

    #[test]
    fn bounded_trace_sink_filters_by_exact_run_oldest_first() {
        let sink = BoundedTraceSink::new(4).unwrap();
        sink.record(event("events/one", Some(7)));
        sink.record(event("events/two", None));
        sink.record(event("events/three", Some(70)));
        sink.record(event("events/four", Some(7)));

        let records = sink.records_for_run(RunId::new(7));

        assert_eq!(
            records
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![0, 3]
        );
        assert!(
            records
                .iter()
                .all(|record| record.event.correlation.run_id == Some(RunId::new(7)))
        );
    }
}
