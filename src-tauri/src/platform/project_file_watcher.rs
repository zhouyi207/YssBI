use crate::application::project_watcher::{
    FileWatcherStartError, ObservedProjectFileChange, ProjectFileChangeSink,
    ProjectFileWatcherDrain, ProjectFileWatcherDrainOutcome, ProjectFileWatcherFactory,
    ProjectFileWatcherSession, ProjectWatcherEpoch, WatcherShutdownControl,
};
use crate::project::{FileChange, FileChangeKind, ProjectRelativePath};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use thiserror::Error;

const PROJECT_FILE_CHANGE_QUEUE_CAPACITY: usize = 1;
const PROJECT_FILE_WATCHER_QUIET_PERIOD: Duration = Duration::from_millis(250);

pub struct NotifyProjectFileWatcher;

impl NotifyProjectFileWatcher {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NotifyProjectFileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectFileWatcherFactory for NotifyProjectFileWatcher {
    fn start(
        &self,
        project_root: &Path,
        epoch: ProjectWatcherEpoch,
        sink: Arc<dyn ProjectFileChangeSink>,
    ) -> Result<Box<dyn ProjectFileWatcherSession>, FileWatcherStartError> {
        let (sender, receiver) = mpsc::sync_channel(PROJECT_FILE_CHANGE_QUEUE_CAPACITY);
        let callback_root = project_root.to_path_buf();
        let callback_sender = sender.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<Event>| match result {
                Ok(event) => {
                    if let Some(change) = file_change_from_event(callback_root.as_path(), &event) {
                        enqueue(
                            &callback_sender,
                            ObservedProjectFileChange { epoch, change },
                        );
                    }
                }
                Err(source) => {
                    let error = NotifyProjectFileWatcherError::Callback(source);
                    tracing::warn!(
                        target: "yssbi::platform::project_file_watcher",
                        diagnostic_domain = "system",
                        diagnostic_event = "watcherError",
                        error = %error,
                        "Project file watcher reported an error"
                    );
                    enqueue(
                        &callback_sender,
                        ObservedProjectFileChange {
                            epoch,
                            change: FileChange::watcher_error(),
                        },
                    );
                }
            })
            .map_err(NotifyProjectFileWatcherError::Create)
            .map_err(report_start_error)?;
        watcher
            .watch(project_root, RecursiveMode::Recursive)
            .map_err(NotifyProjectFileWatcherError::Watch)
            .map_err(report_start_error)?;

        let (completion_sender, completion_receiver) = mpsc::sync_channel(1);
        let worker = spawn_worker(receiver, sink, completion_sender)
            .map_err(|error| report_start_error(NotifyProjectFileWatcherError::Worker(error)))?;
        Ok(Box::new(NotifyProjectFileWatcherSession {
            sender: Some(sender),
            watcher: Some(watcher),
            completion: Some(completion_receiver),
            worker: Some(worker),
        }))
    }
}

fn enqueue(sender: &SyncSender<ObservedProjectFileChange>, change: ObservedProjectFileChange) {
    let _ = sender.try_send(change);
}

fn file_change_from_event(root: &Path, event: &Event) -> Option<FileChange> {
    if !event
        .paths
        .iter()
        .all(|path| path.strip_prefix(root).is_ok())
    {
        return None;
    }

    let kind = file_change_kind(&event.kind);
    event.paths.iter().find_map(|path| {
        let relative = path.strip_prefix(root).ok()?.to_path_buf();
        let relative = ProjectRelativePath::from_observed(relative)?;
        let change = FileChange::new(relative, kind);
        change.is_relevant().then_some(change)
    })
}

fn file_change_kind(kind: &EventKind) -> FileChangeKind {
    match kind {
        EventKind::Create(_) => FileChangeKind::Created,
        EventKind::Modify(_) => FileChangeKind::Modified,
        EventKind::Remove(_) => FileChangeKind::Removed,
        EventKind::Access(_) | EventKind::Other | EventKind::Any => FileChangeKind::Modified,
    }
}

fn spawn_worker(
    receiver: Receiver<ObservedProjectFileChange>,
    sink: Arc<dyn ProjectFileChangeSink>,
    completion: SyncSender<WorkerTerminal>,
) -> Result<JoinHandle<()>, std::io::Error> {
    thread::Builder::new()
        .name("yssbi-project-watcher".into())
        .spawn(move || {
            let terminal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                while let Ok(mut change) = receiver.recv() {
                    loop {
                        match receiver.recv_timeout(PROJECT_FILE_WATCHER_QUIET_PERIOD) {
                            Ok(next) => change = next,
                            Err(mpsc::RecvTimeoutError::Timeout)
                            | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                    sink.publish(change);
                }
            }));
            let terminal = match terminal {
                Ok(()) => WorkerTerminal::Drained,
                Err(_) => WorkerTerminal::Panicked,
            };
            let _ = completion.send(terminal);
        })
}

#[derive(Clone, Copy)]
enum WorkerTerminal {
    Drained,
    Panicked,
}

struct NotifyProjectFileWatcherSession {
    sender: Option<SyncSender<ObservedProjectFileChange>>,
    watcher: Option<RecommendedWatcher>,
    completion: Option<Receiver<WorkerTerminal>>,
    worker: Option<JoinHandle<()>>,
}

impl ProjectFileWatcherSession for NotifyProjectFileWatcherSession {
    fn close_admission(mut self: Box<Self>) -> Box<dyn ProjectFileWatcherDrain> {
        self.sender.take();
        self.watcher.take();
        Box::new(NotifyProjectFileWatcherDrain {
            completion: self.completion.take(),
            worker: self.worker.take(),
        })
    }
}

impl Drop for NotifyProjectFileWatcherSession {
    fn drop(&mut self) {
        self.sender.take();
        self.watcher.take();
        if let Some(worker) = self.worker.take() {
            spawn_worker_reaper(worker);
        }
    }
}

struct NotifyProjectFileWatcherDrain {
    completion: Option<Receiver<WorkerTerminal>>,
    worker: Option<JoinHandle<()>>,
}

impl ProjectFileWatcherDrain for NotifyProjectFileWatcherDrain {
    fn finish(
        mut self: Box<Self>,
        control: WatcherShutdownControl,
    ) -> ProjectFileWatcherDrainOutcome {
        let terminal = match self.completion.as_ref() {
            Some(completion) => match control.remaining() {
                Some(remaining) => match completion.recv_timeout(remaining) {
                    Ok(terminal) => terminal,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        return ProjectFileWatcherDrainOutcome::TimedOut(self);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => WorkerTerminal::Panicked,
                },
                None => match completion.try_recv() {
                    Ok(terminal) => terminal,
                    Err(mpsc::TryRecvError::Empty) => {
                        return ProjectFileWatcherDrainOutcome::TimedOut(self);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => WorkerTerminal::Panicked,
                },
            },
            None => WorkerTerminal::Drained,
        };
        self.completion.take();
        if let Some(worker) = self.worker.take() {
            join_worker(worker);
        }
        match terminal {
            WorkerTerminal::Drained => ProjectFileWatcherDrainOutcome::Drained,
            WorkerTerminal::Panicked => ProjectFileWatcherDrainOutcome::WorkerPanicked,
        }
    }
}

impl Drop for NotifyProjectFileWatcherDrain {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            spawn_worker_reaper(worker);
        }
    }
}

fn join_worker(worker: JoinHandle<()>) {
    let _ = worker.join();
}

fn spawn_worker_reaper(worker: JoinHandle<()>) {
    let _ = thread::Builder::new()
        .name("yssbi-project-watcher-reaper".into())
        .spawn(move || join_worker(worker));
}

#[derive(Debug, Error)]
enum NotifyProjectFileWatcherError {
    #[error("failed to create the project file watcher")]
    Create(#[source] notify::Error),
    #[error("failed to watch the project root")]
    Watch(#[source] notify::Error),
    #[error("project file watcher callback failed")]
    Callback(#[source] notify::Error),
    #[error("failed to spawn the project file watcher worker")]
    Worker(#[source] std::io::Error),
}

fn report_start_error(error: NotifyProjectFileWatcherError) -> FileWatcherStartError {
    tracing::warn!(
        target: "yssbi::platform::project_file_watcher",
        diagnostic_domain = "system",
        diagnostic_event = "watcherStartFailed",
        error = %error,
        "Failed to start project file watcher"
    );
    FileWatcherStartError::StartFailed
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{EventAttributes, ModifyKind};
    use std::path::PathBuf;

    fn path_event(root: &Path, relative: &str) -> Event {
        Event {
            kind: EventKind::Modify(ModifyKind::Any),
            paths: vec![root.join(relative)],
            attrs: EventAttributes::default(),
        }
    }

    #[test]
    fn unrelated_event_does_not_block_a_relevant_change() {
        let root = PathBuf::from(r"C:\project");
        let unrelated = path_event(root.as_path(), "README.md");
        let relevant = path_event(root.as_path(), "events/foo.yssbi-event");

        assert!(file_change_from_event(root.as_path(), &unrelated).is_none());
        assert_eq!(
            file_change_from_event(root.as_path(), &relevant)
                .expect("relevant event is retained")
                .relative_path
                .as_path(),
            Path::new("events/foo.yssbi-event")
        );
    }

    #[test]
    fn observed_paths_outside_the_project_root_are_rejected() {
        let root = PathBuf::from(r"C:\project");
        let outside = path_event(root.as_path(), r"..\other\metadata.yssbi");

        assert!(file_change_from_event(root.as_path(), &outside).is_none());
    }
}
