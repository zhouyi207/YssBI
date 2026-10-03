use super::{ChangeSink, ObservedChange, WatcherEpoch};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct EpochAdmission {
    epoch: WatcherEpoch,
    closed: AtomicBool,
}

impl EpochAdmission {
    pub(super) fn new(epoch: WatcherEpoch) -> Self {
        Self {
            epoch,
            closed: AtomicBool::new(false),
        }
    }

    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    fn admits(&self, epoch: WatcherEpoch) -> bool {
        epoch == self.epoch && !self.closed.load(Ordering::Acquire)
    }
}

pub(super) struct EpochFilteringSink {
    pub(super) admission: Arc<EpochAdmission>,
    pub(super) sink: Arc<dyn ChangeSink>,
}

impl ChangeSink for EpochFilteringSink {
    fn publish(&self, change: ObservedChange) {
        if self.admission.admits(change.epoch) {
            // The source session's drain owns callbacks already admitted here.
            self.sink.publish(change);
        }
    }
}
