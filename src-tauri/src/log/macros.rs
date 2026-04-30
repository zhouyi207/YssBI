//! 日志宏模块
//!
//! 提供类似标准库 log crate 的宏接口：
//! - log_app::trace!(), log_app::debug!(), log_app::info!(), log_app::warn!(), log_app::error!()
//! - log_exec::trace!(), log_exec::debug!(), log_exec::info!(), log_exec::warn!(), log_exec::error!()
//! - log_sys::trace!(), log_sys::debug!(), log_sys::info!(), log_sys::warn!(), log_sys::error!()

/// 应用程序日志宏
pub mod log_app {
    /// Trace 级别日志
    #[macro_export]
    macro_rules! log_app_trace {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_app($crate::log::LogLevel::Trace, format!($($arg)*), None);
            }
        };
    }

    /// Debug 级别日志
    #[macro_export]
    macro_rules! log_app_debug {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_app($crate::log::LogLevel::Debug, format!($($arg)*), None);
            }
        };
    }

    /// Info 级别日志
    #[macro_export]
    macro_rules! log_app_info {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_app($crate::log::LogLevel::Info, format!($($arg)*), None);
            }
        };
    }

    /// Warn 级别日志
    #[macro_export]
    macro_rules! log_app_warn {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_app($crate::log::LogLevel::Warn, format!($($arg)*), None);
            }
        };
    }

    /// Error 级别日志
    #[macro_export]
    macro_rules! log_app_error {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_app($crate::log::LogLevel::Error, format!($($arg)*), None);
            }
        };
    }

    // 重新导出宏，使其可以通过 log_app::info! 调用
    pub use crate::log_app_debug as debug;
    pub use crate::log_app_error as error;
    pub use crate::log_app_info as info;
    pub use crate::log_app_trace as trace;
    pub use crate::log_app_warn as warn;
}

/// 执行日志宏
pub mod log_exec {
    /// Trace 级别日志
    #[macro_export]
    macro_rules! log_exec_trace {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_execution($crate::log::LogLevel::Trace, format!($($arg)*), None);
            }
        };
    }

    /// Debug 级别日志
    #[macro_export]
    macro_rules! log_exec_debug {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_execution($crate::log::LogLevel::Debug, format!($($arg)*), None);
            }
        };
    }

    /// Info 级别日志
    #[macro_export]
    macro_rules! log_exec_info {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_execution($crate::log::LogLevel::Info, format!($($arg)*), None);
            }
        };
    }

    /// Warn 级别日志
    #[macro_export]
    macro_rules! log_exec_warn {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_execution($crate::log::LogLevel::Warn, format!($($arg)*), None);
            }
        };
    }

    /// Error 级别日志
    #[macro_export]
    macro_rules! log_exec_error {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_execution($crate::log::LogLevel::Error, format!($($arg)*), None);
            }
        };
    }

    // 重新导出宏
    pub use crate::log_exec_debug as debug;
    pub use crate::log_exec_error as error;
    pub use crate::log_exec_info as info;
    pub use crate::log_exec_trace as trace;
    pub use crate::log_exec_warn as warn;
}

/// 系统日志宏
pub mod log_sys {
    /// Trace 级别日志
    #[macro_export]
    macro_rules! log_sys_trace {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_system($crate::log::LogLevel::Trace, format!($($arg)*), None);
            }
        };
    }

    /// Debug 级别日志
    #[macro_export]
    macro_rules! log_sys_debug {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_system($crate::log::LogLevel::Debug, format!($($arg)*), None);
            }
        };
    }

    /// Info 级别日志
    #[macro_export]
    macro_rules! log_sys_info {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_system($crate::log::LogLevel::Info, format!($($arg)*), None);
            }
        };
    }

    /// Warn 级别日志
    #[macro_export]
    macro_rules! log_sys_warn {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_system($crate::log::LogLevel::Warn, format!($($arg)*), None);
            }
        };
    }

    /// Error 级别日志
    #[macro_export]
    macro_rules! log_sys_error {
        ($($arg:tt)*) => {
            if let Some(manager) = $crate::log::get_log_manager() {
                manager.log_system($crate::log::LogLevel::Error, format!($($arg)*), None);
            }
        };
    }

    // 重新导出宏
    pub use crate::log_sys_debug as debug;
    pub use crate::log_sys_error as error;
    pub use crate::log_sys_info as info;
    pub use crate::log_sys_trace as trace;
    pub use crate::log_sys_warn as warn;
}
