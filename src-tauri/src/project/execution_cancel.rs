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

/// Registers one execution cancellation token for the lifetime of an operation.
pub struct ExecutionCancelLease<'a> {
    registry: &'a ExecutionCancelRegistry,
    flag: Arc<AtomicBool>,
}

impl ExecutionCancelRegistry {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    pub fn begin(&self) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        *self.active.lock().expect("execution cancel registry lock") = Some(flag.clone());
        flag
    }

    /// Register a cancellation token that is automatically cleared when dropped.
    pub fn lease(&self) -> ExecutionCancelLease<'_> {
        ExecutionCancelLease {
            registry: self,
            flag: self.begin(),
        }
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
        let mut active = self.active.lock().expect("execution cancel registry lock");
        if active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, flag))
        {
            *active = None;
        }
    }
}

impl ExecutionCancelLease<'_> {
    pub fn token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }
}

impl Drop for ExecutionCancelLease<'_> {
    fn drop(&mut self) {
        self.registry.end(&self.flag);
    }
}

impl Default for ExecutionCancelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_clears_active_token_when_operation_returns_early() {
        let registry = ExecutionCancelRegistry::new();
        let token = {
            let lease = registry.lease();
            lease.token()
        };

        registry.cancel_active();

        assert!(!is_execution_cancelled(&token));
    }

    #[test]
    fn older_lease_does_not_clear_newer_active_token() {
        let registry = ExecutionCancelRegistry::new();
        let first = registry.lease();
        let second = registry.lease();
        let second_token = second.token();

        drop(first);
        registry.cancel_active();

        assert!(is_execution_cancelled(&second_token));
    }
}
