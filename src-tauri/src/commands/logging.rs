//! 日志相关命令

use crate::logging::{get_log_manager, read_logs_from_file, count_logs_in_file, LogMessage};
use serde::{Deserialize, Serialize};

/// 日志查询请求
#[derive(Debug, Deserialize)]
pub struct LogQueryRequest {
    pub offset: usize,
    pub limit: usize,
}

/// 日志查询响应
#[derive(Debug, Serialize)]
pub struct LogQueryResponse {
    pub logs: Vec<LogMessage>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
}

/// 获取当前会话的日志（分页）
#[tauri::command]
pub fn get_logs(offset: usize, limit: usize) -> Result<LogQueryResponse, String> {
    let manager = get_log_manager()
        .ok_or_else(|| "Log manager not initialized".to_string())?;
    
    let log_file_path = manager.get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;
    
    let total = count_logs_in_file(&log_file_path)?;
    let logs = read_logs_from_file(&log_file_path, offset, limit)?;
    let has_more = offset + logs.len() < total;
    
    Ok(LogQueryResponse {
        logs,
        total,
        offset,
        limit,
        has_more,
    })
}

/// 获取日志文件路径
#[tauri::command]
pub fn get_log_file_path() -> Result<String, String> {
    let manager = get_log_manager()
        .ok_or_else(|| "Log manager not initialized".to_string())?;
    
    let log_file_path = manager.get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;
    
    Ok(log_file_path.to_string_lossy().to_string())
}

/// 获取日志总数
#[tauri::command]
pub fn get_log_count() -> Result<usize, String> {
    let manager = get_log_manager()
        .ok_or_else(|| "Log manager not initialized".to_string())?;
    
    let log_file_path = manager.get_log_file_path()
        .ok_or_else(|| "No log file available".to_string())?;
    
    count_logs_in_file(&log_file_path)
}
