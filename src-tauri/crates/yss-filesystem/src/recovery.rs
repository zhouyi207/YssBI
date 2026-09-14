use crate::FilesystemError;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct RecoveryMarker {
    state: Arc<Mutex<Option<String>>>,
}

impl RecoveryMarker {
    pub fn mark(&self, message: impl Into<String>) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(message.into());
    }

    pub fn boundary_recovering(&self) -> (std::sync::MutexGuard<'_, Option<String>>, bool) {
        match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        }
    }

    pub fn clear_poison(&self) {
        self.state.clear_poison();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn boundary_is_available(&self) -> bool {
        self.state.try_lock().is_ok()
    }

    pub fn clear(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub fn error(&self) -> Option<FilesystemError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .map(|message| FilesystemError::RecoveryRequired { message })
    }
}
