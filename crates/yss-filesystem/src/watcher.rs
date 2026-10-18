mod admission;
mod drain;
mod lifecycle;
pub mod notify;

use crate::change::FilesystemChange;
pub use lifecycle::WatcherState;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WatcherEpoch(u64);

impl WatcherEpoch {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedChange {
    pub epoch: WatcherEpoch,
    pub change: FilesystemChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherShutdownControl {
    deadline: Instant,
}

impl WatcherShutdownControl {
    pub fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    #[cfg(test)]
    pub(crate) fn is_expired(self) -> bool {
        Instant::now() >= self.deadline
    }

    pub(super) fn after(timeout: Duration) -> Self {
        Self::new(Instant::now() + timeout)
    }

    pub fn remaining(self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum FileWatcherStartError {
    #[error("filesystem watcher failed to start")]
    StartFailed,
}

pub trait ChangeSink: Send + Sync {
    fn publish(&self, change: ObservedChange);
}

pub trait FileWatcherSession: Send {
    /// Stop new source deliveries; the returned drain owns completion of all
    /// callbacks already admitted by this session.
    fn close_admission(self: Box<Self>) -> Box<dyn FileWatcherDrain>;
}

pub enum FileWatcherDrainOutcome {
    Drained,
    WorkerPanicked,
    TimedOut(Box<dyn FileWatcherDrain>),
}

impl fmt::Debug for FileWatcherDrainOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Drained => formatter.write_str("Drained"),
            Self::WorkerPanicked => formatter.write_str("WorkerPanicked"),
            Self::TimedOut(_) => formatter.write_str("TimedOut(<drain>)"),
        }
    }
}

pub trait FileWatcherDrain: Send {
    fn finish(self: Box<Self>, control: WatcherShutdownControl) -> FileWatcherDrainOutcome;
}

pub trait FileWatcherFactory: Send + Sync {
    fn start(
        &self,
        root: &Path,
        epoch: WatcherEpoch,
        sink: Arc<dyn ChangeSink>,
    ) -> Result<Box<dyn FileWatcherSession>, FileWatcherStartError>;
}

#[derive(Error)]
pub enum WatcherError {
    #[error("filesystem watcher failed to start")]
    Start(#[source] FileWatcherStartError),
    #[error("filesystem watcher epoch is exhausted")]
    EpochExhausted,
    #[error("filesystem watcher shutdown timed out")]
    TimedOut(Box<dyn FileWatcherDrain>),
}

impl fmt::Debug for WatcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(error) => formatter.debug_tuple("Start").field(error).finish(),
            Self::EpochExhausted => formatter.write_str("EpochExhausted"),
            Self::TimedOut(_) => formatter.write_str("TimedOut(<drain>)"),
        }
    }
}

#[cfg(test)]
mod tests;
