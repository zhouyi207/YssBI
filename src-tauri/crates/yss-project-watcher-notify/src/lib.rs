//! Native filesystem observation for the platform-neutral project watcher.

#![forbid(unsafe_code)]

use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use thiserror::Error;
use yss_project_change::{ProjectChange, ProjectFileChangeKind, ProjectRelativePath};
use yss_project_watcher::{
    FileWatcherStartError, ObservedProjectChange, ProjectChangeSink, ProjectFileWatcherDrain,
    ProjectFileWatcherDrainOutcome, ProjectFileWatcherFactory, ProjectFileWatcherSession,
    ProjectWatcherEpoch, WatcherShutdownControl,
};

const PROJECT_CHANGE_QUEUE_CAPACITY: usize = 1;
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
        sink: Arc<dyn ProjectChangeSink>,
    ) -> Result<Box<dyn ProjectFileWatcherSession>, FileWatcherStartError> {
        let (sender, receiver) = mpsc::sync_channel(PROJECT_CHANGE_QUEUE_CAPACITY);
        let callback_root = project_root.to_path_buf();
        let callback_sender = sender.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<Event>| match result {
                Ok(event) => {
                    if project_change_from_event(callback_root.as_path(), &event).is_some() {
                        enqueue(&callback_sender);
                    }
                }
                Err(source) => {
                    let error = NotifyProjectFileWatcherError::Callback(source);
                    tracing::warn!(
                        target: "yssbi::project_watcher_notify",
                        diagnostic_domain = "system",
                        diagnostic_event = "watcherError",
                        error = %error,
                        "Project file watcher reported an error"
                    );
                    enqueue(&callback_sender);
                }
            })
            .map_err(NotifyProjectFileWatcherError::Create)
            .map_err(report_start_error)?;
        watcher
            .watch(project_root, RecursiveMode::Recursive)
            .map_err(NotifyProjectFileWatcherError::Watch)
            .map_err(report_start_error)?;

        let (completion_sender, completion_receiver) = mpsc::sync_channel(1);
        let worker = spawn_worker(receiver, epoch, sink, completion_sender)
            .map_err(|error| report_start_error(NotifyProjectFileWatcherError::Worker(error)))?;
        Ok(Box::new(NotifyProjectFileWatcherSession {
            sender: Some(sender),
            watcher: Some(watcher),
            completion: Some(completion_receiver),
            worker: Some(worker),
        }))
    }
}

fn enqueue(sender: &SyncSender<()>) {
    // A full slot already represents a pending rescan of every relevant path.
    let _ = sender.try_send(());
}

fn project_change_from_event(root: &Path, event: &Event) -> Option<ProjectChange> {
    if event.need_rescan() {
        return Some(ProjectChange::rescan_required());
    }
    let kind = project_file_change_kind(&event.kind)?;
    event.paths.iter().find_map(|path| {
        let relative = path.strip_prefix(root).ok()?.to_path_buf();
        let relative = ProjectRelativePath::try_new(relative).ok()?;
        let change = ProjectChange::file(relative, kind);
        change
            .affects_project_index()
            .then_some(ProjectChange::rescan_required())
    })
}

fn project_file_change_kind(kind: &EventKind) -> Option<ProjectFileChangeKind> {
    match kind {
        EventKind::Create(_) => Some(ProjectFileChangeKind::Created),
        EventKind::Modify(ModifyKind::Name(_)) => Some(ProjectFileChangeKind::Renamed),
        EventKind::Modify(_) => Some(ProjectFileChangeKind::Modified),
        EventKind::Remove(_) => Some(ProjectFileChangeKind::Removed),
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
            Some(ProjectFileChangeKind::Modified)
        }
        EventKind::Access(_) => None,
        EventKind::Other | EventKind::Any => Some(ProjectFileChangeKind::Modified),
    }
}

fn spawn_worker(
    receiver: Receiver<()>,
    epoch: ProjectWatcherEpoch,
    sink: Arc<dyn ProjectChangeSink>,
    completion: SyncSender<WorkerTerminal>,
) -> Result<JoinHandle<()>, std::io::Error> {
    thread::Builder::new()
        .name("yssbi-project-watcher".into())
        .spawn(move || {
            let terminal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                while receiver.recv().is_ok() {
                    while receiver
                        .recv_timeout(PROJECT_FILE_WATCHER_QUIET_PERIOD)
                        .is_ok()
                    {}
                    sink.publish(ObservedProjectChange {
                        epoch,
                        change: ProjectChange::rescan_required(),
                    });
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
    sender: Option<SyncSender<()>>,
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
        target: "yssbi::project_watcher_notify",
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
    use notify::event::{EventAttributes, RenameMode};
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

        assert!(project_change_from_event(root.as_path(), &unrelated).is_none());
        assert_eq!(
            project_change_from_event(root.as_path(), &relevant),
            Some(ProjectChange::RescanRequired)
        );
    }

    #[test]
    fn observed_paths_outside_the_project_root_are_rejected() {
        let root = PathBuf::from(r"C:\project");
        let outside = path_event(root.as_path(), r"..\other\metadata.yssbi");

        assert!(project_change_from_event(root.as_path(), &outside).is_none());
    }

    #[test]
    fn read_access_is_ignored_while_write_close_and_rename_are_retained() {
        let root = PathBuf::from(r"C:\project");
        let access = Event {
            kind: EventKind::Access(AccessKind::Read),
            paths: vec![root.join("metadata.yssbi")],
            attrs: EventAttributes::default(),
        };
        let rename = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            paths: vec![root.join("metadata.yssbi")],
            attrs: EventAttributes::default(),
        };
        let write_closed = Event {
            kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
            paths: vec![root.join("metadata.yssbi")],
            attrs: EventAttributes::default(),
        };

        assert!(project_change_from_event(root.as_path(), &access).is_none());
        assert!(project_change_from_event(root.as_path(), &write_closed).is_some());
        assert_eq!(
            project_change_from_event(root.as_path(), &rename),
            Some(ProjectChange::RescanRequired)
        );
    }

    #[test]
    fn mixed_boundary_event_keeps_a_relevant_in_root_path() {
        let root = PathBuf::from(r"C:\project");
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            paths: vec![
                root.join(r"..\other\metadata.yssbi"),
                root.join("metadata.yssbi"),
            ],
            attrs: EventAttributes::default(),
        };

        assert!(project_change_from_event(root.as_path(), &event).is_some());
        let rescan = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert_eq!(
            project_change_from_event(root.as_path(), &rescan),
            Some(ProjectChange::RescanRequired)
        );
    }

    #[test]
    fn pending_rescan_survives_a_full_queue_while_the_sink_is_busy() {
        struct CaptureEpoch(SyncSender<ProjectWatcherEpoch>);
        impl ProjectFileWatcherFactory for CaptureEpoch {
            fn start(
                &self,
                _: &Path,
                epoch: ProjectWatcherEpoch,
                _: Arc<dyn ProjectChangeSink>,
            ) -> Result<Box<dyn ProjectFileWatcherSession>, FileWatcherStartError> {
                self.0.send(epoch).unwrap();
                Err(FileWatcherStartError::StartFailed)
            }
        }
        struct BlockingSink {
            published: SyncSender<ObservedProjectChange>,
            resume: std::sync::Mutex<Receiver<()>>,
        }
        impl ProjectChangeSink for BlockingSink {
            fn publish(&self, change: ObservedProjectChange) {
                let _ = self.published.send(change);
                let _ = self.resume.lock().unwrap().recv();
            }
        }
        let (published, observed) = mpsc::sync_channel(1);
        let (resume, waiting) = mpsc::sync_channel(1);
        let sink = Arc::new(BlockingSink {
            published,
            resume: std::sync::Mutex::new(waiting),
        });
        let (epochs, epoch) = mpsc::sync_channel(1);
        let owner = yss_project_watcher::ProjectWatcherState::new(Arc::new(CaptureEpoch(epochs)));
        assert!(owner.watch_project("test-project", sink.clone()).is_err());
        let epoch = epoch.recv().unwrap();
        let (sender, receiver) = mpsc::sync_channel(PROJECT_CHANGE_QUEUE_CAPACITY);
        let (complete, completion) = mpsc::sync_channel(1);
        let worker = spawn_worker(receiver, epoch, sink, complete).unwrap();
        enqueue(&sender);
        let first = observed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(first.change, ProjectChange::RescanRequired);
        for _ in 0..100 {
            enqueue(&sender);
        }
        resume.send(()).unwrap();
        let second = observed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(second, first);
        drop(sender);
        resume.send(()).unwrap();
        assert!(matches!(
            completion.recv_timeout(Duration::from_secs(3)).unwrap(),
            WorkerTerminal::Drained
        ));
        join_worker(worker);
    }
}
