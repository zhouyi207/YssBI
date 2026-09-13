use crate::plan::PlanSourceIdentity;
use crate::run_registry::RunId;

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
