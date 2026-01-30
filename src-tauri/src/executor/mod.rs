//! 执行器模块
//!
//! 负责图的执行逻辑和运行时管理。

pub mod connection;
pub mod context;
pub mod error;
pub mod node;
pub mod pin;
pub mod processors;
pub mod types;
pub mod value;

// 重新导出常用类型
pub use connection::{Connection, ConnectionManager};
pub use context::ExecutionContext;
pub use error::{
    ConnectionError, ConnectionResult, ExecutionError, ExecutionResult, NodeError, NodeResult,
};
pub use node::{
    get_all_node_definitions, GenericNode, GraphData, Node, NodeData, NodeId, NodeState, PinData,
    PinDefinition, VariableData,
};
pub use pin::{
    BasePin, DataPin, DataPinEvent, DataPinState, ExecPin, ExecPinState, GenericInDataPin,
    GenericInExecPin, GenericOutDataPin, GenericOutExecPin, InDataPin, InExecPin, OutDataPin,
    OutExecPin, PinId, PinType,
};
pub use processors::{DataProcessor, ExecutionContextTrait, FlowProcessor};
pub use types::{DataValue, ExecutionModel};
pub use value::{Value, ValueType};
