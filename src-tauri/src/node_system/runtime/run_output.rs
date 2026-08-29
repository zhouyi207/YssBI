use super::RunId;
use super::execution_event::RunEventSink;
use crate::graph_document::{GraphResourcePath, NodeId, PortAddress};
use std::sync::Mutex;

pub const RUN_OUTPUT_TEXT_MAX_BYTES: usize = 8 * 1024;
pub const RUN_OUTPUT_EVENT_MAX_COUNT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutputEvent {
    pub run_id: RunId,
    pub sequence: u64,
    pub stream: RunOutputStream,
    pub text: Box<str>,
    pub source_graph_path: GraphResourcePath,
    pub source_node_id: NodeId,
    pub source_port: PortAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutputStatus {
    Truncated,
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutputStatusEvent {
    pub run_id: RunId,
    pub sequence: u64,
    pub stream: RunOutputStream,
    pub status: RunOutputStatus,
    pub source_graph_path: GraphResourcePath,
    pub source_node_id: NodeId,
    pub source_port: PortAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutputMessage {
    Output(RunOutputEvent),
    Status(RunOutputStatusEvent),
}

impl RunOutputMessage {
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Output(event) => event.sequence,
            Self::Status(event) => event.sequence,
        }
    }
}

#[derive(Debug)]
struct RunOutputState {
    next_sequence: u64,
    output_count: usize,
    truncated_reported: bool,
    dropped_reported: bool,
}

impl Default for RunOutputState {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            output_count: 0,
            truncated_reported: false,
            dropped_reported: false,
        }
    }
}

pub(crate) trait RunOutputSink: Sync {
    fn emit(
        &self,
        stream: RunOutputStream,
        text: &str,
        source_graph_path: &GraphResourcePath,
        source_node_id: NodeId,
        source_port: &PortAddress,
    );
}

#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct NoopRunOutputSink;

#[cfg(test)]
impl RunOutputSink for NoopRunOutputSink {
    fn emit(&self, _: RunOutputStream, _: &str, _: &GraphResourcePath, _: NodeId, _: &PortAddress) {
    }
}

#[cfg(test)]
pub(crate) static NOOP_RUN_OUTPUT_SINK: NoopRunOutputSink = NoopRunOutputSink;

pub(crate) struct RunOutputEmitter<'a> {
    run_id: RunId,
    events: &'a dyn RunEventSink,
    state: Mutex<RunOutputState>,
}

impl<'a> RunOutputEmitter<'a> {
    pub(crate) fn new(run_id: RunId, events: &'a dyn RunEventSink) -> Self {
        Self {
            run_id,
            events,
            state: Mutex::new(RunOutputState::default()),
        }
    }

    pub(crate) fn emit(
        &self,
        stream: RunOutputStream,
        text: &str,
        source_graph_path: &GraphResourcePath,
        source_node_id: NodeId,
        source_port: &PortAddress,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.output_count >= RUN_OUTPUT_EVENT_MAX_COUNT {
            if !state.dropped_reported {
                state.dropped_reported = true;
                let sequence = take_sequence(&mut state);
                self.events
                    .record_run_output(RunOutputMessage::Status(RunOutputStatusEvent {
                        run_id: self.run_id,
                        sequence,
                        stream,
                        status: RunOutputStatus::Dropped,
                        source_graph_path: source_graph_path.clone(),
                        source_node_id,
                        source_port: source_port.clone(),
                    }));
            }
            return;
        }

        state.output_count += 1;
        let (text, truncated) = bounded_text(text);
        let sequence = take_sequence(&mut state);
        self.events
            .record_run_output(RunOutputMessage::Output(RunOutputEvent {
                run_id: self.run_id,
                sequence,
                stream,
                text,
                source_graph_path: source_graph_path.clone(),
                source_node_id,
                source_port: source_port.clone(),
            }));
        if truncated && !state.truncated_reported {
            state.truncated_reported = true;
            let sequence = take_sequence(&mut state);
            self.events
                .record_run_output(RunOutputMessage::Status(RunOutputStatusEvent {
                    run_id: self.run_id,
                    sequence,
                    stream,
                    status: RunOutputStatus::Truncated,
                    source_graph_path: source_graph_path.clone(),
                    source_node_id,
                    source_port: source_port.clone(),
                }));
        }
    }
}

impl RunOutputSink for RunOutputEmitter<'_> {
    fn emit(
        &self,
        stream: RunOutputStream,
        text: &str,
        source_graph_path: &GraphResourcePath,
        source_node_id: NodeId,
        source_port: &PortAddress,
    ) {
        RunOutputEmitter::emit(
            self,
            stream,
            text,
            source_graph_path,
            source_node_id,
            source_port,
        );
    }
}

fn take_sequence(state: &mut RunOutputState) -> u64 {
    let sequence = state.next_sequence;
    state.next_sequence += 1;
    sequence
}

fn bounded_text(text: &str) -> (Box<str>, bool) {
    if text.len() <= RUN_OUTPUT_TEXT_MAX_BYTES {
        return (text.into(), false);
    }
    let mut end = RUN_OUTPUT_TEXT_MAX_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].into(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::protocol::PortKey;
    use crate::node_system::runtime::RunEvent;

    #[derive(Default)]
    struct OutputEvents(std::sync::Mutex<Vec<RunOutputMessage>>);

    impl RunEventSink for OutputEvents {
        fn record(&self, _: RunEvent) {}

        fn record_run_output(&self, event: RunOutputMessage) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[test]
    fn run_output_is_bounded_and_reports_each_limit_once() {
        let events = OutputEvents::default();
        let output = RunOutputEmitter::new(RunId::new(99), &events);
        let source_graph_path = GraphResourcePath::new("events/output.yssbi-event").unwrap();
        let source_node_id = NodeId::from_uuid(uuid::Uuid::nil());
        let source_port = PortAddress::declared(
            source_node_id,
            PortKey::new("message").expect("static Print port key is valid"),
        );
        let oversized = "界".repeat(RUN_OUTPUT_TEXT_MAX_BYTES / 3 + 2);

        output.emit(
            RunOutputStream::Stdout,
            &oversized,
            &source_graph_path,
            source_node_id,
            &source_port,
        );
        for index in 1..=RUN_OUTPUT_EVENT_MAX_COUNT + 2 {
            output.emit(
                RunOutputStream::Stdout,
                &format!("event-{index}"),
                &source_graph_path,
                source_node_id,
                &source_port,
            );
        }

        let messages = events.0.lock().unwrap();
        assert_eq!(messages.len(), RUN_OUTPUT_EVENT_MAX_COUNT + 2);
        assert_eq!(
            messages
                .iter()
                .map(RunOutputMessage::sequence)
                .collect::<Vec<_>>(),
            (1..=(RUN_OUTPUT_EVENT_MAX_COUNT as u64 + 2)).collect::<Vec<_>>()
        );
        let output_events = messages
            .iter()
            .filter_map(|message| match message {
                RunOutputMessage::Output(event) => Some(event),
                RunOutputMessage::Status(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(output_events.len(), RUN_OUTPUT_EVENT_MAX_COUNT);
        assert_eq!(output_events[0].source_graph_path, source_graph_path);
        assert_eq!(output_events[0].source_port, source_port);
        assert!(output_events[0].text.len() <= RUN_OUTPUT_TEXT_MAX_BYTES);
        assert_ne!(output_events[0].text.as_ref(), oversized);
        let statuses = messages
            .iter()
            .filter_map(|message| match message {
                RunOutputMessage::Output(_) => None,
                RunOutputMessage::Status(event) => Some(event.status),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            statuses,
            [RunOutputStatus::Truncated, RunOutputStatus::Dropped]
        );
    }
}
