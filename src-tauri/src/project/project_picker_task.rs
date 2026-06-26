use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const PICKER_TASK_CANCELLED: &str = "PICKER_TASK_CANCELLED";

pub fn picker_task_cancelled_error() -> String {
    PICKER_TASK_CANCELLED.to_string()
}

pub fn is_picker_task_cancelled(cancel: &AtomicBool) -> bool {
    cancel.load(Ordering::Relaxed)
}

/// 跟踪项目选择页进行中的可取消任务（扫描、清理等）。
pub struct ProjectPickerTaskCancelRegistry {
    active: Mutex<Option<Arc<AtomicBool>>>,
}

impl ProjectPickerTaskCancelRegistry {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    pub fn begin(&self) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        *self.active
            .lock()
            .expect("project picker task cancel registry lock") = Some(flag.clone());
        flag
    }

    pub fn cancel_active(&self) {
        if let Some(flag) = self
            .active
            .lock()
            .expect("project picker task cancel registry lock")
            .as_ref()
        {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn end(&self, flag: &Arc<AtomicBool>) {
        let mut active = self
            .active
            .lock()
            .expect("project picker task cancel registry lock");
        if active.as_ref().is_some_and(|current| Arc::ptr_eq(current, flag)) {
            *active = None;
        }
    }
}

impl Default for ProjectPickerTaskCancelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProjectCleanupProgressEvent {
    Checking { current: usize, total: usize },
    Removing { removed: usize, total: usize },
}
