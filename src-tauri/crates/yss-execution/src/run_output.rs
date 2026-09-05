use crate::plan::PlanSourceIdentity;
use crate::run_registry::RunId;

pub const RUN_OUTPUT_MAX_RECORDS: usize = 256;
pub const RUN_OUTPUT_MAX_TEXT_BYTES: usize = 16 * 1024;
pub const RUN_OUTPUT_MAX_TOTAL_BYTES: usize = 512 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("run output requires an explicit node and port source")]
pub struct InvalidRunOutputSource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunOutputSummary {
    pub last_sequence: u64,
    pub emitted_records: usize,
    pub emitted_bytes: usize,
    pub truncated: bool,
    pub dropped: bool,
}

pub struct RunOutputEmitter {
    run_id: RunId,
    next_sequence: u64,
    emitted_records: usize,
    emitted_bytes: usize,
    truncated: bool,
    dropped: bool,
}

impl RunOutputEmitter {
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            next_sequence: 1,
            emitted_records: 0,
            emitted_bytes: 0,
            truncated: false,
            dropped: false,
        }
    }

    /// Produces a bounded batch for the execution adapter's nonblocking output channel.
    pub fn emit(
        &mut self,
        stream: RunOutputStream,
        text: &str,
        source: &PlanSourceIdentity,
    ) -> Result<Vec<RunOutputMessage>, InvalidRunOutputSource> {
        if source.node().is_none() || source.port().is_none() {
            return Err(InvalidRunOutputSource);
        }
        let mut messages = Vec::new();
        let remaining = RUN_OUTPUT_MAX_TOTAL_BYTES - self.emitted_bytes;
        if self.emitted_records >= RUN_OUTPUT_MAX_RECORDS || remaining == 0 {
            if !self.dropped {
                self.dropped = true;
                messages.push(self.status(stream, RunOutputStatus::Dropped, source));
            }
            return Ok(messages);
        }
        let mut end = text.len().min(RUN_OUTPUT_MAX_TEXT_BYTES).min(remaining);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if end > 0 || text.is_empty() {
            let sequence = self.next_sequence;
            self.next_sequence += 1;
            self.emitted_records += 1;
            self.emitted_bytes += end;
            messages.push(RunOutputMessage::Output(RunOutputEvent {
                run_id: self.run_id,
                sequence,
                stream,
                text: text[..end].into(),
                source: source.clone(),
            }));
        }
        if end < text.len() && !self.truncated {
            self.truncated = true;
            messages.push(self.status(stream, RunOutputStatus::Truncated, source));
        }
        Ok(messages)
    }

    pub const fn summary(&self) -> RunOutputSummary {
        RunOutputSummary {
            last_sequence: self.next_sequence - 1,
            emitted_records: self.emitted_records,
            emitted_bytes: self.emitted_bytes,
            truncated: self.truncated,
            dropped: self.dropped,
        }
    }

    fn status(
        &mut self,
        stream: RunOutputStream,
        status: RunOutputStatus,
        source: &PlanSourceIdentity,
    ) -> RunOutputMessage {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        RunOutputMessage::Status(RunOutputStatusEvent {
            run_id: self.run_id,
            sequence,
            stream,
            status,
            source: source.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunOutputEvent {
    run_id: RunId,
    sequence: u64,
    stream: RunOutputStream,
    text: Box<str>,
    source: PlanSourceIdentity,
}

impl RunOutputEvent {
    pub const fn run_id(&self) -> RunId {
        self.run_id
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn stream(&self) -> RunOutputStream {
        self.stream
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOutputStatus {
    Truncated,
    Dropped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunOutputStatusEvent {
    run_id: RunId,
    sequence: u64,
    stream: RunOutputStream,
    status: RunOutputStatus,
    source: PlanSourceIdentity,
}

impl RunOutputStatusEvent {
    pub const fn run_id(&self) -> RunId {
        self.run_id
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn stream(&self) -> RunOutputStream {
        self.stream
    }

    pub const fn status(&self) -> RunOutputStatus {
        self.status
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunOutputMessage {
    Output(RunOutputEvent),
    Status(RunOutputStatusEvent),
}

impl RunOutputMessage {
    pub const fn run_id(&self) -> RunId {
        match self {
            Self::Output(event) => event.run_id(),
            Self::Status(event) => event.run_id(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Output(event) => event.sequence(),
            Self::Status(event) => event.sequence(),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;

    pub fn output(
        run_id: RunId,
        sequence: u64,
        stream: RunOutputStream,
        text: impl Into<Box<str>>,
        source: PlanSourceIdentity,
    ) -> RunOutputMessage {
        RunOutputMessage::Output(RunOutputEvent {
            run_id,
            sequence,
            stream,
            text: text.into(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanGraphId, PlanNodeId, PlanPortAddress};

    fn source() -> PlanSourceIdentity {
        PlanSourceIdentity::new(
            PlanGraphId::new("events/output.yssbi-event".into()).unwrap(),
            Some(PlanNodeId::new("source-node".into()).unwrap()),
            Some(PlanPortAddress::new("stdout".into()).unwrap()),
        )
    }

    #[test]
    fn output_emitter_bounds_unicode_and_allocates_one_order_for_text_and_status() {
        let source = source();
        let run = RunId::from_existing(7);
        let mut emitter = RunOutputEmitter::new(run);
        let mut messages = emitter
            .emit(
                RunOutputStream::Stdout,
                &"世".repeat(RUN_OUTPUT_MAX_TEXT_BYTES),
                &source,
            )
            .unwrap();
        for _ in 0..RUN_OUTPUT_MAX_RECORDS + 4 {
            messages.extend(
                emitter
                    .emit(RunOutputStream::Stderr, "next", &source)
                    .unwrap(),
            );
        }
        assert_eq!(messages.len(), RUN_OUTPUT_MAX_RECORDS + 2);
        for (index, message) in messages.iter().enumerate() {
            assert_eq!(message.run_id(), run);
            assert_eq!(message.sequence(), index as u64 + 1);
            if let RunOutputMessage::Output(output) = message {
                assert_eq!(output.source(), &source);
                assert!(output.text().len() <= RUN_OUTPUT_MAX_TEXT_BYTES);
            }
        }
        assert!(emitter.summary().truncated);
        assert!(emitter.summary().dropped);
        assert!(emitter.summary().emitted_bytes <= RUN_OUTPUT_MAX_TOTAL_BYTES);
    }

    #[test]
    fn output_emitter_rejects_incomplete_sources_before_advancing() {
        let mut emitter = RunOutputEmitter::new(RunId::from_existing(1));
        let incomplete = PlanSourceIdentity::new(
            PlanGraphId::new("events/output.yssbi-event".into()).unwrap(),
            None,
            None,
        );
        assert_eq!(
            emitter.emit(RunOutputStream::Stdout, "payload", &incomplete),
            Err(InvalidRunOutputSource)
        );
        assert_eq!(emitter.summary().last_sequence, 0);
    }
}
