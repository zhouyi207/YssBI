//! Pin 实例（运行时）
//!
//! Pin 实例由 Graph 管理，不属于 Node。
//! Pin 不存储连接信息，所有连接由 ConnectionManager 管理。

use super::{
    pin_payload::PinPayload, DataPinState, ExecPinState, PinDefinition, PinDirection, PinId,
    PinKind, PinState, PinOrder
};
use crate::executor::node::NodeId;
use crate::executor::value::DataValue;
use serde::{Deserialize, Serialize};

/// Pin 实例（运行时）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    /// Pin ID
    pub id: PinId,

    /// 所属节点 ID
    pub node_id: NodeId,

    /// 静态定义（真·蓝图）
    pub definition: PinDefinition,

    /// Pin 状态
    pub state: PinState,

    /// 运行时负载
    pub payload: PinPayload,

    /// 顺序
    pub order: PinOrder,
}

impl PinInstance {
    /// 从定义创建实例
    pub fn from_definition(def: &PinDefinition, node_id: NodeId, order: i32) -> Self {
        let id = PinId::new();

        let state = match def.kind {
            PinKind::Data => PinState::Data(DataPinState::default()),
            PinKind::Exec => PinState::Exec(ExecPinState::default()),
        };

        Self {
            id,
            node_id,
            definition: def.clone(),
            state,
            payload: PinPayload::new(def),
            order: PinOrder(order)
        }
    }

    /// 获取有效值（优先用户值，其次当前值）
    pub fn effective_value(&self) -> Option<&DataValue> {
        match &self.payload {
            PinPayload::Data { user_value, value } => user_value.as_ref().or(value.as_ref()),
            PinPayload::Exec => None,
        }
    }

    /// 设置运行时值（来自连接）
    pub fn set_value(&mut self, value: DataValue) {
        match &mut self.payload {
            PinPayload::Data { value: v, .. } => {
                *v = Some(value);
                if let PinState::Data(state) = &mut self.state {
                    *state = DataPinState::Ready;
                }
            }
            PinPayload::Exec => {}
        }
    }

    /// 设置用户值（未连接时）
    pub fn set_user_value(&mut self, value: Option<DataValue>) {
        if let PinPayload::Data { user_value, .. } = &mut self.payload {
            *user_value = value;
        }
    }

    /// 清除运行时值（断线 / 重置）
    pub fn clear_value(&mut self) {
        if let PinPayload::Data { value, .. } = &mut self.payload {
            *value = None;
            if let PinState::Data(state) = &mut self.state {
                *state = DataPinState::Uninitialized;
            }
        }
    }

    /// 是否为数据 Pin
    pub fn is_data(&self) -> bool {
        matches!(self.definition.kind, PinKind::Data)
    }

    /// 是否为执行 Pin
    pub fn is_exec(&self) -> bool {
        matches!(self.definition.kind, PinKind::Exec)
    }

    /// 是否为输入 Pin
    pub fn is_input(&self) -> bool {
        matches!(self.definition.direction, PinDirection::Input)
    }

    /// 是否为输出 Pin
    pub fn is_output(&self) -> bool {
        matches!(self.definition.direction, PinDirection::Output)
    }
}
