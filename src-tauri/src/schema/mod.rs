//! Schema 模块

pub mod connection;
pub mod database;
pub mod graph;
pub mod history;
pub mod node;
pub mod pin;
pub mod project;
pub mod value;
pub mod variables;

pub use connection::*;
pub use database::*;
pub use graph::*;
pub use history::*;
pub use node::*;
pub use pin::*;
pub use project::*;
pub use value::*;
pub use variables::*;

use serde::Serialize;

/// 完整的 Schema 数据，用于一次性传输给前端
#[derive(Debug, Clone, Serialize)]
pub struct EditorSchema {}

/// 获取完整的 Schema
pub fn get_editor_schema() -> EditorSchema {
    EditorSchema {}
}
