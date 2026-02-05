use super::{LogLevel, LogType};
use serde::{Deserialize, Serialize};

/// 日志消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    /// 时间
    pub timestamp: String,
    /// 等级
    pub level: LogLevel,
    /// 日志类型
    pub log_type: LogType,
    /// 消息内容
    pub message: String,
    /// 来源
    pub source: Option<String>, // 日志来源（如节点ID、模块名等）
}
