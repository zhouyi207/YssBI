//! 节点模块
//!
//! 包含节点定义、类型和处理器。

pub mod definition;
pub mod processors;
pub mod registry;
pub mod types;

// 重新导出常用类型
pub use definition::NodeDefinition;
pub use processors::{DataProcessor, ExecutionContextTrait, FlowProcessor};
pub use registry::get_all_node_definitions;
pub use types::{GraphData, NodeData, PinData, PinDefinition, VariableData};
