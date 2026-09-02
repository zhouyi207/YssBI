use crate::plan::PlanSourceIdentity;
use crate::run_registry::RunId;

pub const RUN_OUTPUT_TEXT_MAX_BYTES: usize = 8 * 1024;
pub const RUN_OUTPUT_EVENT_MAX_COUNT: usize = 256;

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

pub(crate) struct RunOutputEmitter<'a> {
    run_id: RunId,
    next_sequence: u64,
    output_count: usize,
    truncated_reported: bool,
    dropped_reported: bool,
    deliver: &'a mut dyn FnMut(RunOutputMessage),
}

impl<'a> RunOutputEmitter<'a> {
    pub(crate) fn new(run_id: RunId, deliver: &'a mut dyn FnMut(RunOutputMessage)) -> Self {
        Self {
            run_id,
            next_sequence: 1,
            output_count: 0,
            truncated_reported: false,
            dropped_reported: false,
            deliver,
        }
    }

    pub(crate) fn emit(
        &mut self,
        stream: RunOutputStream,
        text: &str,
        source: &PlanSourceIdentity,
    ) -> bool {
        if self.output_count >= RUN_OUTPUT_EVENT_MAX_COUNT {
            if !self.dropped_reported {
                self.dropped_reported = true;
                let Some(sequence) = self.take_sequence() else {
                    return false;
                };
                (self.deliver)(RunOutputMessage::Status(RunOutputStatusEvent {
                    run_id: self.run_id,
                    sequence,
                    stream,
                    status: RunOutputStatus::Dropped,
                    source: source.clone(),
                }));
            }
            return true;
        }

        self.output_count += 1;
        let (text, truncated) = bounded_text(text);
        let Some(sequence) = self.take_sequence() else {
            return false;
        };
        (self.deliver)(RunOutputMessage::Output(RunOutputEvent {
            run_id: self.run_id,
            sequence,
            stream,
            text,
            source: source.clone(),
        }));
        if truncated && !self.truncated_reported {
            self.truncated_reported = true;
            let Some(sequence) = self.take_sequence() else {
                return false;
            };
            (self.deliver)(RunOutputMessage::Status(RunOutputStatusEvent {
                run_id: self.run_id,
                sequence,
                stream,
                status: RunOutputStatus::Truncated,
                source: source.clone(),
            }));
        }
        true
    }

    fn take_sequence(&mut self) -> Option<u64> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.checked_add(1)?;
        Some(sequence)
    }
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
            PlanGraphId::from_existing("events/output.yssbi-event".into()),
            Some(PlanNodeId::from_existing("node".into())),
            Some(PlanPortAddress::from_existing("node:message".into())),
        )
    }

    #[test]
    fn output_is_bounded_and_reports_each_limit_once() {
        let mut messages = Vec::new();
        let mut deliver = |message| messages.push(message);
        let oversized = "界".repeat(RUN_OUTPUT_TEXT_MAX_BYTES / 3 + 2);
        {
            let mut emitter = RunOutputEmitter::new(RunId::from_existing(1), &mut deliver);
            assert!(emitter.emit(RunOutputStream::Stdout, &oversized, &source()));
            for index in 1..=RUN_OUTPUT_EVENT_MAX_COUNT + 2 {
                assert!(emitter.emit(
                    RunOutputStream::Stdout,
                    &format!("event-{index}"),
                    &source(),
                ));
            }
        }

        assert_eq!(messages.len(), RUN_OUTPUT_EVENT_MAX_COUNT + 2);
        assert_eq!(
            messages
                .iter()
                .map(RunOutputMessage::sequence)
                .collect::<Vec<_>>(),
            (1..=(RUN_OUTPUT_EVENT_MAX_COUNT as u64 + 2)).collect::<Vec<_>>()
        );
        let output = messages
            .iter()
            .filter_map(|message| match message {
                RunOutputMessage::Output(event) => Some(event),
                RunOutputMessage::Status(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(output.len(), RUN_OUTPUT_EVENT_MAX_COUNT);
        assert!(output[0].text().len() <= RUN_OUTPUT_TEXT_MAX_BYTES);
        assert_ne!(output[0].text(), oversized);
        assert_eq!(
            messages
                .iter()
                .filter_map(|message| match message {
                    RunOutputMessage::Output(_) => None,
                    RunOutputMessage::Status(event) => Some(event.status()),
                })
                .collect::<Vec<_>>(),
            [RunOutputStatus::Truncated, RunOutputStatus::Dropped]
        );
    }
}
