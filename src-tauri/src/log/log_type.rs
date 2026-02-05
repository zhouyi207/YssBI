use serde::{Deserialize, Serialize};

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
