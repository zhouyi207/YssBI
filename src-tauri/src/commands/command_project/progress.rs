use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;

use crate::project::{
    ProjectCleanupProgress, ProjectProgress, ProjectProgressSink, ProjectScanProgress,
};

pub const PROJECT_PROGRESS_QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectProgressDto {
    Scan(ProjectScanProgressDto),
    Cleanup(ProjectCleanupProgressDto),
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectScanProgressDto {
    Scanning,
    Discovered { count: usize },
    Registering { current: usize, total: usize },
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectCleanupProgressDto {
    Checking { current: usize, total: usize },
    Removing { removed: usize, total: usize },
}

fn to_dto(progress: ProjectProgress) -> ProjectProgressDto {
    match progress {
        ProjectProgress::Scan(progress) => ProjectProgressDto::Scan(match progress {
            ProjectScanProgress::Scanning => ProjectScanProgressDto::Scanning,
            ProjectScanProgress::Discovered { count } => {
                ProjectScanProgressDto::Discovered { count }
            }
            ProjectScanProgress::Registering { current, total } => {
                ProjectScanProgressDto::Registering { current, total }
            }
        }),
        ProjectProgress::Cleanup(progress) => ProjectProgressDto::Cleanup(match progress {
            ProjectCleanupProgress::Checking { current, total } => {
                ProjectCleanupProgressDto::Checking { current, total }
            }
            ProjectCleanupProgress::Removing { removed, total } => {
                ProjectCleanupProgressDto::Removing { removed, total }
            }
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressEnqueueOutcome {
    Accepted,
    ReceiverClosed,
    DroppedAtCapacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressAdapterShutdownControl {
    deadline: Instant,
}

impl ProgressAdapterShutdownControl {
    pub const fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    fn remaining(self) -> Option<Duration> {
        self.deadline.checked_duration_since(Instant::now())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressAdapterTerminalError {
    DeliveryFailed,
    Panicked,
}

struct ProjectProgressPublisherState {
    closed: bool,
    sender: Option<SyncSender<ProjectProgress>>,
}

pub struct ProjectProgressPublisher {
    state: Mutex<ProjectProgressPublisherState>,
}

impl ProjectProgressPublisher {
    pub fn close(&self) {
        let sender = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.closed = true;
            state.sender.take()
        };
        drop(sender);
    }

    pub fn enqueue(&self, progress: ProjectProgress) -> ProgressEnqueueOutcome {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.closed {
            return ProgressEnqueueOutcome::ReceiverClosed;
        }
        let Some(sender) = state.sender.as_ref() else {
            return ProgressEnqueueOutcome::ReceiverClosed;
        };
        match sender.try_send(progress) {
            Ok(()) => ProgressEnqueueOutcome::Accepted,
            Err(TrySendError::Full(_)) => ProgressEnqueueOutcome::DroppedAtCapacity,
            Err(TrySendError::Disconnected(_)) => ProgressEnqueueOutcome::ReceiverClosed,
        }
    }
}

impl ProjectProgressSink for ProjectProgressPublisher {
    fn publish(&self, progress: ProjectProgress) {
        let _ = self.enqueue(progress);
    }
}

enum ProjectProgressWorkerTerminal {
    Drained,
    DeliveryFailed,
    Panicked,
}

pub struct ProjectProgressWorker {
    completion: Receiver<ProjectProgressWorkerTerminal>,
    join: Option<JoinHandle<()>>,
}

pub enum ProjectProgressDrainOutcome {
    Drained(Result<(), ProgressAdapterTerminalError>),
    TimedOut(ProjectProgressWorker),
}

impl ProjectProgressWorker {
    pub fn finish(
        mut self,
        control: ProgressAdapterShutdownControl,
    ) -> ProjectProgressDrainOutcome {
        let terminal = match control.remaining() {
            Some(remaining) => match self.completion.recv_timeout(remaining) {
                Ok(terminal) => terminal,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return ProjectProgressDrainOutcome::TimedOut(self);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    ProjectProgressWorkerTerminal::Panicked
                }
            },
            None => match self.completion.try_recv() {
                Ok(terminal) => terminal,
                Err(mpsc::TryRecvError::Empty) => {
                    return ProjectProgressDrainOutcome::TimedOut(self);
                }
                Err(mpsc::TryRecvError::Disconnected) => ProjectProgressWorkerTerminal::Panicked,
            },
        };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        ProjectProgressDrainOutcome::Drained(match terminal {
            ProjectProgressWorkerTerminal::Drained => Ok(()),
            ProjectProgressWorkerTerminal::DeliveryFailed => {
                Err(ProgressAdapterTerminalError::DeliveryFailed)
            }
            ProjectProgressWorkerTerminal::Panicked => Err(ProgressAdapterTerminalError::Panicked),
        })
    }
}

pub fn bounded_project_progress_adapter(
    channel: Channel<ProjectProgressDto>,
) -> (Arc<ProjectProgressPublisher>, ProjectProgressWorker) {
    let (sender, receiver) = mpsc::sync_channel(PROJECT_PROGRESS_QUEUE_CAPACITY);
    let publisher = Arc::new(ProjectProgressPublisher {
        state: Mutex::new(ProjectProgressPublisherState {
            closed: false,
            sender: Some(sender),
        }),
    });
    let (completion_sender, completion) = mpsc::sync_channel(1);
    let join = thread::Builder::new()
        .name("yssbi-project-progress".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut delivery_failed = false;
                while let Ok(progress) = receiver.recv() {
                    if channel.send(to_dto(progress)).is_err() {
                        delivery_failed = true;
                    }
                }
                if delivery_failed {
                    ProjectProgressWorkerTerminal::DeliveryFailed
                } else {
                    ProjectProgressWorkerTerminal::Drained
                }
            }));
            let terminal = result.unwrap_or(ProjectProgressWorkerTerminal::Panicked);
            let _ = completion_sender.send(terminal);
        })
        .expect("project progress worker must spawn");
    (
        publisher,
        ProjectProgressWorker {
            completion,
            join: Some(join),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publisher_drops_newest_when_capacity_is_full_and_closes_all_clones() {
        let (sender, receiver) = mpsc::sync_channel(PROJECT_PROGRESS_QUEUE_CAPACITY);
        let publisher = Arc::new(ProjectProgressPublisher {
            state: Mutex::new(ProjectProgressPublisherState {
                closed: false,
                sender: Some(sender),
            }),
        });
        for _ in 0..PROJECT_PROGRESS_QUEUE_CAPACITY {
            assert_eq!(
                publisher.enqueue(ProjectProgress::Scan(ProjectScanProgress::Scanning)),
                ProgressEnqueueOutcome::Accepted
            );
        }
        assert_eq!(
            publisher.enqueue(ProjectProgress::Scan(ProjectScanProgress::Scanning)),
            ProgressEnqueueOutcome::DroppedAtCapacity
        );
        let clone = publisher.clone();
        publisher.close();
        assert_eq!(
            clone.enqueue(ProjectProgress::Scan(ProjectScanProgress::Scanning)),
            ProgressEnqueueOutcome::ReceiverClosed
        );
        drop(clone);
        drop(publisher);
        assert_eq!(receiver.try_iter().count(), PROJECT_PROGRESS_QUEUE_CAPACITY);
    }
}
