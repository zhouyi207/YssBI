//! Graph 实现
//!
//! ❌ 不负责「定义」
//! ❌ 不负责「编译策略」
//! ❌ 不负责「执行调度」
//! ✅ 持有状态
//! ✅ 提供受控 mutation API

use super::{GraphDataState, GraphKind, GraphPosition};
use crate::graph::connection::Connection;
use crate::graph::node::OutputSchemaContext;
pub use crate::graph::node::SchemaProvider;
use crate::graph::node::{
    ColumnSchema, DataSchema, NodeDefinition, NodeId, NodeInstance, NodeInstanceParams,
    NodePosition, PinResolverContext,
};
use crate::graph::pin::{
    DataRole, PinDataTypeDefinition, PinDefinition, PinDirection, PinId, PinInstance, PinKind,
    PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use crate::graph::value::DataValue;
use crate::graph::{GraphId, TypeVarDefinition, TypeVarId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Graph（运行时世界）
///
/// Graph 是唯一的运行时真实来源，管理：
/// - 所有 Node, Pin 实例 和连接关系
/// - 类型推断上下文
#[derive(Clone)]
pub struct GraphInstance {
    // 图 id
    pub id: GraphId,

    // 图 name
    pub name: String,

    // 类型
    pub kind: GraphKind,

    // 位置
    pub position: GraphPosition,

    // Function graph 对外签名。Event 始终为空。
    pub function_inputs: Vec<FunctionSignaturePin>,
    pub function_outputs: Vec<FunctionSignaturePin>,

    pub runtime_prepared_epoch: u64,

    // 数据状态 (node, pin, connection)
    pub data_state: Arc<RwLock<GraphDataState>>,

    // 节点类型注册表（不持久化，需要在加载后重新设置）
    registry: Arc<NodeRegistry>,

    // 模式提供器（不持久化，运行时通过 ProjectState 注入）
    schema_provider: Option<SchemaProvider>,
}

mod connections;
mod dynamic_pins;
mod infer;
mod lifecycle;
mod nodes;
mod persistence;
mod pins;
mod repeatable_pins;
mod schema;
mod symbols;
mod types;

pub use types::*;
