use serde::{Deserialize, Serialize};
use std::fmt;

/// 日志类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogType {
    /// 应用程序日志（项目操作、设置、窗口管理）
    Application,
    /// 执行日志（图执行、节点执行状态）
    Execution,
    /// 系统日志（初始化、插件加载、内部系统操作）
    System,
    /// 图编辑日志（节点、连线、Pin 等结构操作）
    Graph,
    /// 数据日志（数据库、变量、DataFrame 操作）
    Data,
}

impl fmt::Display for LogType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogType::Application => write!(f, "app"),
            LogType::Execution => write!(f, "exec"),
            LogType::System => write!(f, "sys"),
            LogType::Graph => write!(f, "graph"),
            LogType::Data => write!(f, "data"),
        }
    }
}
