//! 执行器模块
//!
//! 包含节点图的执行引擎，遵循以下架构原则：
//! - Graph 是运行时的唯一真实来源（Single Source of Truth）
//! - Node 仅作为定义/行为模板，不持有运行态状态
//! - Pin 不属于 Node，由 Graph 统一管理
//! - Executor 以 Graph + NodeId 为中心运行

pub mod context;
pub mod data_store;
pub mod engine;
pub mod presentation;
pub mod runtime_source_invalidation;
pub mod source_builder;
pub mod struct_json;
pub mod window_data_store;

pub use context::*;
pub use data_store::*;
pub use engine::*;
pub use presentation::*;
pub use runtime_source_invalidation::*;
pub use source_builder::*;
pub use struct_json::*;
pub use window_data_store::*;
