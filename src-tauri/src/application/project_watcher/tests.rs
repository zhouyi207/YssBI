use super::*;
use crate::project::{FileChange, FileChangeKind, ProjectRelativePath};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Barrier, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

#[derive(Default)]
struct FakeFactory {
    sessions: Mutex<Vec<Arc<FakeSessionControl>>>,
}

struct FakeSessionControl {
    epoch: ProjectWatcherEpoch,
    sink: Arc<dyn ProjectFileChangeSink>,
}

impl FakeFactory {
    fn emit_after_barrier(
        &self,
        session_index: usize,
        change: FileChange,
        ready: Arc<Barrier>,
        release: Arc<Barrier>,
    ) -> JoinHandle<()> {
        let control = self
            .sessions
            .lock()
            .unwrap()
            .get(session_index)
            .cloned()
            .expect("test watcher session exists");
        thread::spawn(move || {
            ready.wait();
            release.wait();
            control.sink.publish(ObservedProjectFileChange {
                epoch: control.epoch,
                change,
            });
        })
    }

    fn emit(&self, session_index: usize, change: FileChange) {
        let control = self
            .sessions
            .lock()
            .unwrap()
            .get(session_index)
            .cloned()
            .expect("test watcher session exists");
        control.sink.publish(ObservedProjectFileChange {
            epoch: control.epoch,
            change,
        });
    }
}

impl ProjectFileWatcherFactory for FakeFactory {
    fn start(
        &self,
        _project_root: &Path,
        epoch: ProjectWatcherEpoch,
        sink: Arc<dyn ProjectFileChangeSink>,
    ) -> Result<Box<dyn ProjectFileWatcherSession>, FileWatcherStartError> {
        self.sessions
            .lock()
            .unwrap()
            .push(Arc::new(FakeSessionControl { epoch, sink }));
        Ok(Box::new(FakeSession))
    }
}

struct FakeSession;

impl ProjectFileWatcherSession for FakeSession {
    fn close_admission(self: Box<Self>) -> Box<dyn ProjectFileWatcherDrain> {
        Box::new(ImmediateDrain)
    }
}

struct ImmediateDrain;

impl ProjectFileWatcherDrain for ImmediateDrain {
    fn finish(self: Box<Self>, _control: WatcherShutdownControl) -> ProjectFileWatcherDrainOutcome {
        ProjectFileWatcherDrainOutcome::Drained
    }
}

struct RecordingSink {
    changes: Mutex<Vec<FileChange>>,
}

impl RecordingSink {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            changes: Mutex::new(Vec::new()),
        })
    }
}

impl ProjectFileChangeSink for RecordingSink {
    fn publish(&self, change: ObservedProjectFileChange) {
        self.changes.lock().unwrap().push(change.change);
    }
}

fn relevant_change() -> FileChange {
    FileChange::new(
        ProjectRelativePath::new("events/changed.yssbi-event"),
        FileChangeKind::Modified,
    )
}

#[test]
fn replacement_drops_old_watcher_changes_before_project_reconciliation() {
    let factory = Arc::new(FakeFactory::default());
    let state = ProjectWatcherState::for_test(factory.clone());
    let sink = RecordingSink::new();

    state
        .watch_project("C:/project/metadata.yssbi", sink.clone())
        .unwrap();

    let ready = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let old_change =
        factory.emit_after_barrier(0, relevant_change(), ready.clone(), release.clone());
    ready.wait();

    state
        .watch_project("C:/project/metadata.yssbi", sink.clone())
        .unwrap();

    release.wait();
    old_change.join().unwrap();
    assert!(sink.changes.lock().unwrap().is_empty());

    factory.emit(1, relevant_change());
    assert_eq!(sink.changes.lock().unwrap().len(), 1);
}

struct BlockingDrain {
    worker: Option<JoinHandle<()>>,
    worker_done: Arc<AtomicBool>,
    join_count: Arc<AtomicUsize>,
    joined: Sender<()>,
}

impl ProjectFileWatcherDrain for BlockingDrain {
    fn finish(
        mut self: Box<Self>,
        control: WatcherShutdownControl,
    ) -> ProjectFileWatcherDrainOutcome {
        if control.is_expired() && !self.worker_done.load(Ordering::Acquire) {
            return ProjectFileWatcherDrainOutcome::TimedOut(self);
        }

        self.worker
            .take()
            .expect("the sole worker join owner exists")
            .join()
            .unwrap();
        self.join_count.fetch_add(1, Ordering::AcqRel);
        self.joined.send(()).unwrap();
        ProjectFileWatcherDrainOutcome::Drained
    }
}

struct BlockingSession {
    close_count: Arc<AtomicUsize>,
    drain: Option<BlockingDrain>,
}

impl ProjectFileWatcherSession for BlockingSession {
    fn close_admission(mut self: Box<Self>) -> Box<dyn ProjectFileWatcherDrain> {
        self.close_count.fetch_add(1, Ordering::AcqRel);
        Box::new(self.drain.take().expect("test drain is present"))
    }
}

#[test]
fn watcher_shutdown_timeout_retains_handle_and_retry_joins_worker() {
    let (release_tx, release_rx) = mpsc::channel();
    let (worker_done_tx, worker_done_rx) = mpsc::channel();
    let (joined_tx, joined_rx): (Sender<()>, Receiver<()>) = mpsc::channel();
    let worker_done = Arc::new(AtomicBool::new(false));
    let worker_done_for_thread = worker_done.clone();
    let worker = thread::spawn(move || {
        release_rx.recv().unwrap();
        worker_done_for_thread.store(true, Ordering::Release);
        worker_done_tx.send(()).unwrap();
    });
    let join_count = Arc::new(AtomicUsize::new(0));
    let close_count = Arc::new(AtomicUsize::new(0));
    let session: Box<dyn ProjectFileWatcherSession> = Box::new(BlockingSession {
        close_count: close_count.clone(),
        drain: Some(BlockingDrain {
            worker: Some(worker),
            worker_done,
            join_count: join_count.clone(),
            joined: joined_tx,
        }),
    });

    let drain = session.close_admission();
    assert_eq!(close_count.load(Ordering::Acquire), 1);
    let drain = match drain.finish(WatcherShutdownControl::new(Instant::now())) {
        ProjectFileWatcherDrainOutcome::TimedOut(drain) => drain,
        ProjectFileWatcherDrainOutcome::Drained
        | ProjectFileWatcherDrainOutcome::DeliveryFailed
        | ProjectFileWatcherDrainOutcome::WorkerPanicked => {
            panic!("the blocked worker must retain its drain owner")
        }
    };
    assert_eq!(join_count.load(Ordering::Acquire), 0);

    release_tx.send(()).unwrap();
    worker_done_rx.recv().unwrap();
    assert!(matches!(
        drain.finish(WatcherShutdownControl::new(Instant::now())),
        ProjectFileWatcherDrainOutcome::Drained
    ));
    joined_rx.recv().unwrap();
    assert_eq!(join_count.load(Ordering::Acquire), 1);
}
