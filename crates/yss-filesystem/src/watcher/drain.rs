use super::{
    FileWatcherDrain, FileWatcherDrainOutcome, FileWatcherSession, WatcherShutdownControl,
};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

const WATCHER_REAPER_TIMEOUT: Duration = Duration::from_secs(1);

enum DrainOwner {
    Session(Box<dyn FileWatcherSession>),
    Drain(Box<dyn FileWatcherDrain>),
}

enum DrainState {
    Pending(DrainOwner),
    Finishing,
    Drained,
    WorkerPanicked,
}

pub(super) struct DrainCell {
    state: Mutex<DrainState>,
    changed: Condvar,
}

impl DrainCell {
    pub(super) fn new(session: Box<dyn FileWatcherSession>) -> Self {
        Self {
            state: Mutex::new(DrainState::Pending(DrainOwner::Session(session))),
            changed: Condvar::new(),
        }
    }

    fn handle(self: &Arc<Self>) -> Box<dyn FileWatcherDrain> {
        Box::new(DrainHandle {
            cell: Arc::clone(self),
        })
    }

    pub(super) fn finish(
        self: &Arc<Self>,
        control: WatcherShutdownControl,
    ) -> FileWatcherDrainOutcome {
        let owner = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                match &*state {
                    DrainState::Pending(_) => {
                        let DrainState::Pending(owner) =
                            std::mem::replace(&mut *state, DrainState::Finishing)
                        else {
                            unreachable!("pending drain owner was just checked")
                        };
                        break owner;
                    }
                    DrainState::Finishing => {
                        let Some(remaining) = control.remaining() else {
                            return FileWatcherDrainOutcome::TimedOut(self.handle());
                        };
                        (state, _) = self
                            .changed
                            .wait_timeout(state, remaining)
                            .unwrap_or_else(PoisonError::into_inner);
                    }
                    DrainState::Drained => return FileWatcherDrainOutcome::Drained,
                    DrainState::WorkerPanicked => return FileWatcherDrainOutcome::WorkerPanicked,
                }
            }
        };

        // Closing and joining are source callbacks. Neither runs with our state
        // lock held, and an unwind must not leave concurrent retries stranded.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let drain = match owner {
                DrainOwner::Session(session) => session.close_admission(),
                DrainOwner::Drain(drain) => drain,
            };
            drain.finish(control)
        }))
        .unwrap_or(FileWatcherDrainOutcome::WorkerPanicked);
        let (next, outcome) = match outcome {
            FileWatcherDrainOutcome::TimedOut(owner) => (
                DrainState::Pending(DrainOwner::Drain(owner)),
                FileWatcherDrainOutcome::TimedOut(self.handle()),
            ),
            FileWatcherDrainOutcome::Drained => {
                (DrainState::Drained, FileWatcherDrainOutcome::Drained)
            }
            FileWatcherDrainOutcome::WorkerPanicked => (
                DrainState::WorkerPanicked,
                FileWatcherDrainOutcome::WorkerPanicked,
            ),
        };
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) = next;
        self.changed.notify_all();
        outcome
    }
}

struct DrainHandle {
    cell: Arc<DrainCell>,
}

impl FileWatcherDrain for DrainHandle {
    fn finish(self: Box<Self>, control: WatcherShutdownControl) -> FileWatcherDrainOutcome {
        self.cell.finish(control)
    }
}

pub(super) fn spawn_drain_reaper(cell: Arc<DrainCell>) {
    let _ = thread::Builder::new()
        .name("yss-filesystem-watcher-reaper".into())
        .spawn(move || {
            while let FileWatcherDrainOutcome::TimedOut(_) =
                cell.finish(WatcherShutdownControl::after(WATCHER_REAPER_TIMEOUT))
            {}
        });
}
