use serde::{Deserialize, Serialize};
use std::fmt;

/// 日志类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
    /// 应用程序日志
    Application,
    /// 执行日志
    Execution,
    /// 系统日志
    System,
}

impl fmt::Display for LogType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogType::Application => write!(f, "app"),
            LogType::Execution => write!(f, "exec"),
            LogType::System => write!(f, "sys"),
        }
    }
}
