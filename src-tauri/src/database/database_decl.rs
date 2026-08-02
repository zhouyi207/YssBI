use super::DatabaseEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDecl {
    pub id: String,
    pub engine: DatabaseEngine,
    pub schema_version: u32,
    pub required: bool,
    /// 显示名称（unique name），导入时由后端生成，用于 EditorView 与 DataViewer 同步
    #[serde(default)]
    pub name: Option<String>,
}
