//! 执行器模块
//!
//! 负责图的执行逻辑和运行时管理。

pub mod context;
pub mod types;
pub mod error;
pub mod pin;
pub mod node;
pub mod processors;
pub mod connection;
pub mod graph;

// 重新导出常用类型
pub use context::ExecutionContext;
pub use types::DataValue;
pub use pin::{
    PinId, DataPinState, ExecPinState, DataPinEvent, PinType,
    BasePin, DataPin, InDataPin, OutDataPin, ExecPin, InExecPin, OutExecPin,
    GenericInDataPin, GenericOutDataPin, GenericInExecPin, GenericOutExecPin
};
pub use processors::{DataProcessor, FlowProcessor, ExecutionContextTrait};
pub use node::{
    NodeId, NodeState, Node, GenericNode,
    NodeData, PinData, GraphData, PinDefinition, VariableData,
    get_all_node_definitions
};
pub use error::{
    NodeError, ConnectionError, ExecutionError,
    NodeResult, ConnectionResult, ExecutionResult
};
pub use connection::{ConnectionManager, Connection};
pub use graph::RuntimeGraph;
