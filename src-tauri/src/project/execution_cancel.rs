use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const EXECUTION_CANCELLED: &str = "EXECUTION_CANCELLED";

pub fn execution_cancelled_error() -> String {
    EXECUTION_CANCELLED.to_string()
}

pub fn is_execution_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed)
}

/// Tracks the in-flight `execute_project` task so the UI can request cancellation.
pub struct ExecutionCancelRegistry {
    active: Mutex<Option<Arc<AtomicBool>>>,
}

impl ExecutionCancelRegistry {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    pub fn begin(&self) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        *self
            .active
            .lock()
            .expect("execution cancel registry lock") = Some(flag.clone());
        flag
    }

    pub fn cancel_active(&self) {
        if let Some(flag) = self
            .active
            .lock()
            .expect("execution cancel registry lock")
            .as_ref()
        {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn end(&self, flag: &Arc<AtomicBool>) {
        let mut active = self
            .active
            .lock()
            .expect("execution cancel registry lock");
        if active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, flag))
        {
            *active = None;
        }
    }
}

impl Default for ExecutionCancelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
