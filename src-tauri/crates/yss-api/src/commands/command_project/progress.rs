use std::io;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::ipc::Channel;

use yss_project_progress::{
    ProjectCleanupProgress, ProjectProgress, ProjectProgressSink, ProjectScanProgress,
};

pub const PROJECT_PROGRESS_QUEUE_CAPACITY: usize = 64;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectProgressDto {
    Scanning,
    Discovered { count: usize },
    Registering { current: usize, total: usize },
    Checking { current: usize, total: usize },
    Removing { removed: usize, total: usize },
}

fn to_dto(progress: ProjectProgress) -> ProjectProgressDto {
    match progress {
        ProjectProgress::Scan(ProjectScanProgress::Scanning) => ProjectProgressDto::Scanning,
        ProjectProgress::Scan(ProjectScanProgress::Discovered { count }) => {
            ProjectProgressDto::Discovered { count }
        }
        ProjectProgress::Scan(ProjectScanProgress::Registering { current, total }) => {
            ProjectProgressDto::Registering { current, total }
        }
        ProjectProgress::Cleanup(ProjectCleanupProgress::Checking { current, total }) => {
            ProjectProgressDto::Checking { current, total }
        }
        ProjectProgress::Cleanup(ProjectCleanupProgress::Removing { removed, total }) => {
            ProjectProgressDto::Removing { removed, total }
        }
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

#[derive(Debug, thiserror::Error)]
#[error("project progress worker failed to spawn")]
pub(crate) struct ProjectProgressAdapterSpawnError {
    #[source]
    source: io::Error,
}

impl From<io::Error> for ProjectProgressAdapterSpawnError {
    fn from(source: io::Error) -> Self {
        Self { source }
    }
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

pub(crate) fn reap_project_progress_worker(mut worker: ProjectProgressWorker) {
    let _ = thread::Builder::new()
        .name("yssbi-project-progress-reaper".into())
        .spawn(move || {
            while let ProjectProgressDrainOutcome::TimedOut(next) = worker.finish(
                ProgressAdapterShutdownControl::new(Instant::now() + Duration::from_secs(1)),
            ) {
                worker = next;
            }
        });
}

type ProjectProgressWorkerTask = Box<dyn FnOnce() + Send + 'static>;

pub(crate) fn bounded_project_progress_adapter(
    channel: Channel<ProjectProgressDto>,
) -> Result<(Arc<ProjectProgressPublisher>, ProjectProgressWorker), ProjectProgressAdapterSpawnError>
{
    bounded_project_progress_adapter_with(channel, |task| {
        thread::Builder::new()
            .name("yssbi-project-progress".into())
            .spawn(task)
    })
}

fn bounded_project_progress_adapter_with(
    channel: Channel<ProjectProgressDto>,
    spawn: impl FnOnce(ProjectProgressWorkerTask) -> io::Result<JoinHandle<()>>,
) -> Result<(Arc<ProjectProgressPublisher>, ProjectProgressWorker), ProjectProgressAdapterSpawnError>
{
    let (sender, receiver) = mpsc::sync_channel(PROJECT_PROGRESS_QUEUE_CAPACITY);
    let publisher = Arc::new(ProjectProgressPublisher {
        state: Mutex::new(ProjectProgressPublisherState {
            closed: false,
            sender: Some(sender),
        }),
    });
    let (completion_sender, completion) = mpsc::sync_channel(1);
    let task: ProjectProgressWorkerTask = Box::new(move || {
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
    });
    let join = spawn(task)?;
    Ok((
        publisher,
        ProjectProgressWorker {
            completion,
            join: Some(join),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_returns_typed_error_when_worker_spawn_fails() {
        let result = bounded_project_progress_adapter_with(Channel::new(|_| Ok(())), |_task| {
            Err(io::Error::other("injected spawn failure"))
        });

        let Err(error) = result else {
            panic!("injected worker spawn failure must be returned");
        };
        assert_eq!(error.to_string(), "project progress worker failed to spawn");
        assert_eq!(error.source.kind(), io::ErrorKind::Other);
    }

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
