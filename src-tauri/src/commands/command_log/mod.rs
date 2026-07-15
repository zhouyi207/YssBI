use crate::log::log_manager::{count_logs_in_file, get_log_manager, read_logs_from_file};
use crate::log::{LogLevel, LogMessage, LogType};
use crate::error::AppError;

/// 前端日志入口：前端所有日志通过此命令发送到 Rust LogManager，
/// 统一写入文件 + 终端 + emit("log-message") 返回给前端 LogWindow。
#[tauri::command]
pub fn frontend_log(level: LogLevel, log_type: LogType, message: String, source: Option<String>) {
    if let Some(manager) = get_log_manager() {
        let log = LogMessage {
            timestamp: chrono::Local::now()
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
            level,
            log_type,
            message,
            source,
        };
        manager.emit_log(log);
    }
}

/// 获取日志条目（分页，从末尾开始读取）
///
/// offset: 从末尾开始的偏移量（0 = 最新）
/// limit: 要读取的条数（默认 100）
#[tauri::command]
pub fn get_logs(offset: Option<usize>, limit: Option<usize>) -> Result<Vec<LogMessage>, AppError> {
    let manager = get_log_manager().ok_or_else(|| "Log manager not initialized".to_string())?;

    let file_path = manager
        .get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;

    Ok(read_logs_from_file(&file_path, offset.unwrap_or(0), limit.unwrap_or(100))?)
}

/// 获取当前日志文件路径
#[tauri::command]
pub fn get_log_file_path() -> Result<String, AppError> {
    let manager = get_log_manager().ok_or_else(|| "Log manager not initialized".to_string())?;

    Ok(manager
        .get_log_file_path()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "No log file available".to_string())?)
}

/// 获取日志文件中的总日志条数
#[tauri::command]
pub fn get_log_count() -> Result<usize, AppError> {
    let manager = get_log_manager().ok_or_else(|| "Log manager not initialized".to_string())?;

    let file_path = manager
        .get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;

    Ok(count_logs_in_file(&file_path)?)
}
