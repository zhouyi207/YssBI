//! Schema 模块

pub mod connection;
pub mod database;
pub mod graph;
pub mod history;
pub mod node;
pub mod pin;
pub mod project;
pub mod variables;

pub use connection::*;
pub use database::*;
pub use graph::*;
pub use history::*;
pub use node::*;
pub use pin::*;
pub use project::*;
pub use variables::*;

use crate::graph::value::TypeSystemSnapshot;
use serde::Serialize;

/// 完整的 Schema 数据，用于初始化时一次性传输给前端
/// 包含节点定义（含 pin 的 metaData，如 dropdown 的 widget_options）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSchema {
    /// 节点定义列表，含完整 pin 槽位及 metaData（widgetType、widgetOptions 等）
    pub node_definitions: Vec<NodeDefinitionDTO>,
    /// 类型系统快照，前端用于镜像后端 pin 类型匹配规则。
    pub type_system: TypeSystemSnapshot,
}
