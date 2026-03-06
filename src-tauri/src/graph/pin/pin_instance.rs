/// Pin 实例（运行时）
///
/// Pin 实例由 Graph 管理，不属于 Node。
/// Pin 不存储连接信息，所有连接由 ConnectionManager 管理。
use super::{PinDefinition, PinDirection, PinId, PinKind, PinOrder};
use crate::graph::node::DataSchema;
use crate::graph::{DataValue, NodeId, TypeVarId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinInstance {
    pub id: PinId,
    pub node_id: NodeId,
    pub definition: PinDefinition,
    pub order: PinOrder,
    pub type_var_id: Option<TypeVarId>,
    pub user_value: Option<DataValue>,
    /// 连接时传播的 schema（output pin 存储输出 schema，供下游 input 使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_schema: Option<DataSchema>,
}

impl PinInstance {
    /// 从定义创建实例
    pub fn from_definition(def: &PinDefinition, node_id: NodeId, order: i32) -> Self {
        Self {
            id: PinId::new(),
            node_id,
            definition: def.clone(),
            order: PinOrder(order),
            type_var_id: None,
            user_value: None,
            resolved_schema: None,
        }
    }

    pub fn with_type_var_id(mut self, type_var_id: Option<TypeVarId>) -> Self {
        self.type_var_id = type_var_id;
        self
    }

    pub fn is_data(&self) -> bool {
        matches!(self.definition.kind, PinKind::Data)
    }

    pub fn is_exec(&self) -> bool {
        matches!(self.definition.kind, PinKind::Exec)
    }

    pub fn is_input(&self) -> bool {
        matches!(self.definition.direction, PinDirection::Input)
    }

    pub fn is_output(&self) -> bool {
        matches!(self.definition.direction, PinDirection::Output)
    }
}
