//! Node 模块
//!
//! Node 仅作为定义/行为模板，不持有运行态状态。
//! 所有运行态数据由 Graph 管理。

pub mod node_id;
pub mod node_definition;
pub mod node_instance;
pub mod node_processor;
pub mod node_registry;
pub mod catalog;

pub use node_definition::*;
pub use node_instance::*;
pub use node_processor::*;
pub use node_registry::*;
pub use node_id::*;