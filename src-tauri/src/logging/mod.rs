//! 日志模块

use tauri::AppHandle;

static mut LOG_MANAGER: Option<LogManager> = None;

pub struct LogManager {
    _app_handle: AppHandle,
}

impl LogManager {
    fn new(app_handle: AppHandle) -> Self {
        Self {
            _app_handle: app_handle,
        }
    }
}

pub fn init_log_manager(app_handle: AppHandle) {
    unsafe {
        LOG_MANAGER = Some(LogManager::new(app_handle));
    }
}
