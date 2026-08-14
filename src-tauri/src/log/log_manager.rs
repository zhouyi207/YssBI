use super::{LogLevel, LogMessage, LogType};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

pub struct LogManager {
    app_handle: Option<Arc<Mutex<AppHandle>>>,
    log_file: Arc<Mutex<Option<File>>>,
    log_file_path: Arc<Mutex<Option<PathBuf>>>,
}

impl LogManager {
    pub fn new() -> Self {
        Self {
            app_handle: None,
            log_file: Arc::new(Mutex::new(None)),
            log_file_path: Arc::new(Mutex::new(None)),
        }
    }

    /// 设置 AppHandle
    pub fn set_app_handle(&mut self, handle: AppHandle) {
        self.app_handle = Some(Arc::new(Mutex::new(handle)));

        // 初始化日志文件
        self.init_log_file();
    }

    /// 初始化日志文件
    fn init_log_file(&self) {
        // 创建 logs 目录
        if let Some(handle) = &self.app_handle {
            let handle = handle.lock().unwrap();

            let logs_dir = if cfg!(debug_assertions) {
                PathBuf::from("..").join("logs")
            } else {
                handle
                    .path()
                    .app_log_dir()
                    .expect("Failed to get app log dir")
            };

            std::fs::create_dir_all(&logs_dir).ok();

            let filename = format!("app_{}.log", chrono::Local::now().format("%Y%m%d_%H%M%S"));

            let log_path = logs_dir.join(filename);

            match OpenOptions::new().create(true).append(true).open(&log_path) {
                Ok(file) => {
                    *self.log_file.lock().unwrap() = Some(file);
                    *self.log_file_path.lock().unwrap() = Some(log_path);
                }
                Err(e) => {
                    eprintln!("Failed to create log file: {}", e);
                }
            }
        }
    }

    /// 写入日志到文件
    fn write_to_file(&self, log: &LogMessage) {
        if let Some(file) = self.log_file.lock().unwrap().as_mut() {
            let json = serde_json::to_string(log).unwrap_or_default();
            let _ = writeln!(file, "{}", json);
            let _ = file.flush();
        }
    }

    /// 发送日志到前端
    pub fn emit_log(&self, log: LogMessage) {
        // 输出到终端（使用 Tauri 日志插件）
        // 格式：[类型] 消息
        let message = format!(
            "[{}] {}",
            log.log_type.to_string().to_uppercase(),
            log.message
        );

        // 根据日志级别选择对应的输出宏
        match log.level {
            LogLevel::Trace => tauri_plugin_log::log::trace!("{}", message),
            LogLevel::Debug => tauri_plugin_log::log::debug!("{}", message),
            LogLevel::Info => tauri_plugin_log::log::info!("{}", message),
            LogLevel::Warn => tauri_plugin_log::log::warn!("{}", message),
            LogLevel::Error => tauri_plugin_log::log::error!("{}", message),
        }

        // 写入文件
        self.write_to_file(&log);

        // 发送到前端
        if let Some(handle) = &self.app_handle {
            if let Ok(handle) = handle.lock() {
                let _ = handle.emit("log-message", &log);
            }
        }
    }

    /// 发送应用程序日志
    pub fn log_app(&self, level: LogLevel, message: String, source: Option<String>) {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type: LogType::Application,
            message,
            source,
        };
        self.emit_log(log);
    }

    /// 发送执行日志
    pub fn log_execution(&self, level: LogLevel, message: String, source: Option<String>) {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type: LogType::Execution,
            message,
            source,
        };
        self.emit_log(log);
    }

    /// 发送系统日志
    pub fn log_system(&self, level: LogLevel, message: String, source: Option<String>) {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type: LogType::System,
            message,
            source,
        };
        self.emit_log(log);
    }

    /// 发送图编辑日志
    pub fn log_graph(&self, level: LogLevel, message: String, source: Option<String>) {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type: LogType::Graph,
            message,
            source,
        };
        self.emit_log(log);
    }

    /// 发送数据操作日志
    pub fn log_data(&self, level: LogLevel, message: String, source: Option<String>) {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type: LogType::Data,
            message,
            source,
        };
        self.emit_log(log);
    }

    /// 发送用户通知日志
    pub fn log_notify(&self, level: LogLevel, message: String, source: Option<String>) {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type: LogType::Notify,
            message,
            source,
        };
        self.emit_log(log);
    }

    /// 获取当前日志文件路径
    pub fn get_log_file_path(&self) -> Option<PathBuf> {
        self.log_file_path.lock().unwrap().clone()
    }
}

// 全局日志管理器实例（使用 OnceLock 替代 static mut）
static LOG_MANAGER: OnceLock<LogManager> = OnceLock::new();

/// 初始化日志管理器
pub fn init_log_manager(app_handle: AppHandle) {
    let mut manager = LogManager::new();
    manager.set_app_handle(app_handle);
    let _ = LOG_MANAGER.set(manager);
}

/// 获取日志管理器
pub fn get_log_manager() -> Option<&'static LogManager> {
    LOG_MANAGER.get()
}

pub fn emit_execution_log(level: LogLevel, message: String, source: Option<String>) {
    #[cfg(test)]
    TEST_LOGS.with(|logs| {
        logs.borrow_mut().push(LogMessage {
            timestamp: String::new(),
            level,
            log_type: LogType::Execution,
            message: message.clone(),
            source: source.clone(),
        });
    });

    if let Some(manager) = get_log_manager() {
        manager.log_execution(level, message, source);
    }
}

pub fn emit_notify_log(level: LogLevel, message: String, source: Option<String>) {
    #[cfg(test)]
    TEST_LOGS.with(|logs| {
        logs.borrow_mut().push(LogMessage {
            timestamp: String::new(),
            level,
            log_type: LogType::Notify,
            message: message.clone(),
            source: source.clone(),
        });
    });

    if let Some(manager) = get_log_manager() {
        manager.log_notify(level, message, source);
    }
}

#[cfg(test)]
thread_local! {
    static TEST_LOGS: std::cell::RefCell<Vec<LogMessage>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn clear_test_logs() {
    TEST_LOGS.with(|logs| logs.borrow_mut().clear());
}

#[cfg(test)]
pub(crate) fn take_test_logs() -> Vec<LogMessage> {
    TEST_LOGS.with(|logs| std::mem::take(&mut *logs.borrow_mut()))
}

/// 读取日志文件（分页，从末尾开始）
/// offset: 从末尾开始的偏移量（0 表示最新的日志）
/// limit: 要读取的日志数量
pub fn read_logs_from_file(
    file_path: &PathBuf,
    offset: usize,
    limit: usize,
) -> Result<Vec<LogMessage>, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open log file: {}", e))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().filter_map(|line| line.ok()).collect();

    let total = all_lines.len();

    // 如果 offset 超过总数，返回空
    if offset >= total {
        return Ok(Vec::new());
    }

    // 计算从末尾开始的起始位置
    let start = if offset + limit > total {
        0
    } else {
        total - offset - limit
    };

    let end = total - offset;

    let mut logs = Vec::new();
    for line in &all_lines[start..end] {
        if let Ok(log) = serde_json::from_str::<LogMessage>(line) {
            logs.push(log);
        }
    }

    Ok(logs)
}

/// 获取日志文件总行数
pub fn count_logs_in_file(file_path: &PathBuf) -> Result<usize, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open log file: {}", e))?;

    let reader = BufReader::new(file);
    Ok(reader.lines().count())
}

/// 便捷宏：发送应用程序日志
#[macro_export]
macro_rules! log_app {
    ($level:expr, $($arg:tt)*) => {
        if let Some(manager) = $crate::log::get_log_manager() {
            manager.log_app($level, format!($($arg)*), None);
        }
    };
}

/// 便捷宏：发送执行日志
#[macro_export]
macro_rules! log_exec {
    ($level:expr, $($arg:tt)*) => {
        if let Some(manager) = $crate::log::get_log_manager() {
            manager.log_execution($level, format!($($arg)*), None);
        }
    };
}

/// 便捷宏：发送系统日志
#[macro_export]
macro_rules! log_sys {
    ($level:expr, $($arg:tt)*) => {
        if let Some(manager) = $crate::log::get_log_manager() {
            manager.log_system($level, format!($($arg)*), None);
        }
    };
}

/// 便捷宏：发送图编辑日志
#[macro_export]
macro_rules! log_graph {
    ($level:expr, $($arg:tt)*) => {
        if let Some(manager) = $crate::log::get_log_manager() {
            manager.log_graph($level, format!($($arg)*), None);
        }
    };
}

/// 便捷宏：发送数据操作日志
#[macro_export]
macro_rules! log_data {
    ($level:expr, $($arg:tt)*) => {
        if let Some(manager) = $crate::log::get_log_manager() {
            manager.log_data($level, format!($($arg)*), None);
        }
    };
}
