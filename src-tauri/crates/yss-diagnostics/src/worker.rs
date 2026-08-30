use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnqueueResult {
    Enqueued,
    Full,
    Closed,
}

pub(crate) struct BoundedWorker<T> {
    sender: SyncSender<T>,
    active: Arc<AtomicBool>,
}

impl<T> BoundedWorker<T>
where
    T: Send + 'static,
{
    pub(crate) fn spawn(
        name: impl Into<String>,
        capacity: usize,
        mut handle: impl FnMut(T) -> bool + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(capacity.max(1));
        let active = Arc::new(AtomicBool::new(true));
        let worker_active = active.clone();
        let worker = thread::Builder::new().name(name.into()).spawn(move || {
            while worker_active.load(Ordering::Acquire) {
                let Ok(value) = receiver.recv() else {
                    break;
                };
                if !worker_active.load(Ordering::Acquire) {
                    break;
                }
                let keep_running =
                    catch_unwind(AssertUnwindSafe(|| handle(value))).unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
            worker_active.store(false, Ordering::Release);
        })?;
        drop(worker);
        Ok(Self { sender, active })
    }

    pub(crate) fn try_enqueue(&self, value: T) -> EnqueueResult {
        if !self.active.load(Ordering::Acquire) {
            return EnqueueResult::Closed;
        }
        match self.sender.try_send(value) {
            Ok(()) => EnqueueResult::Enqueued,
            Err(TrySendError::Full(_)) => EnqueueResult::Full,
            Err(TrySendError::Disconnected(_)) => {
                self.active.store(false, Ordering::Release);
                EnqueueResult::Closed
            }
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
    }
}

impl<T> Drop for BoundedWorker<T> {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}
