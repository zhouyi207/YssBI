use crate::log::log_manager::{get_log_manager, read_logs_from_file, count_logs_in_file};
use crate::log::LogMessage;

/// 获取日志条目（分页，从末尾开始读取）
///
/// offset: 从末尾开始的偏移量（0 = 最新）
/// limit: 要读取的条数（默认 100）
#[tauri::command]
pub fn get_logs(offset: Option<usize>, limit: Option<usize>) -> Result<Vec<LogMessage>, String> {
    let manager = get_log_manager()
        .ok_or_else(|| "Log manager not initialized".to_string())?;

    let file_path = manager.get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;

    read_logs_from_file(&file_path, offset.unwrap_or(0), limit.unwrap_or(100))
}

/// 获取当前日志文件路径
#[tauri::command]
pub fn get_log_file_path() -> Result<String, String> {
    let manager = get_log_manager()
        .ok_or_else(|| "Log manager not initialized".to_string())?;

    manager.get_log_file_path()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "No log file available".to_string())
}

/// 获取日志文件中的总日志条数
#[tauri::command]
pub fn get_log_count() -> Result<usize, String> {
    let manager = get_log_manager()
        .ok_or_else(|| "Log manager not initialized".to_string())?;

    let file_path = manager.get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;

    count_logs_in_file(&file_path)
}
