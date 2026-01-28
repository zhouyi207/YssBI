//! 项目状态管理模块
//!
//! 提供全局状态管理，作为数据的 Single Source of Truth。
//! 前端通过 Tauri 命令进行 CRUD 操作，通过 Tauri Events 接收数据变更通知。

// 导出模块
mod dataframe_crud;
mod events;
#[macro_use]
pub mod macros;
mod node_crud;
mod project_state;
mod subgraph_crud;
mod variable_crud;

// 重新导出公共接口
pub use events::{emit_project_event, ProjectEvent};
pub use project_state::ProjectState;

// 导出宏（在 crate 根级别使用 #[macro_export]）
// 宏已在 macros.rs 中使用 #[macro_export] 导出
