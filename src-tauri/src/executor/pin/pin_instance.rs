/// Pin 实例（运行时）
///
/// Pin 实例由 Graph 管理，不属于 Node。
/// Pin 不存储连接信息，所有连接由 ConnectionManager 管理。

use super::{
    PinRuntime, DataPinState, ExecPinState, PinDefinition, PinDirection, PinId,
    PinKind, PinState, PinOrder,
};
use crate::executor::node::NodeId;
use crate::executor::value::DataValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    pub id: PinId,
    pub node_id: NodeId,
    pub definition: PinDefinition,
    pub state: PinState,
    pub runtime: PinRuntime,
    pub order: PinOrder,
}

impl PinInstance {
    /// 从定义创建实例
    pub fn from_definition(def: &PinDefinition, node_id: NodeId, order: i32) -> Self {
        let id = PinId::new();

        let state = match def.kind {
            PinKind::Data => PinState::Data(DataPinState::Uninitialized),
            PinKind::Exec => PinState::Exec(ExecPinState::default()),
        };

        Self {
            id,
            node_id,
            definition: def.clone(),
            state,
            runtime: PinRuntime::new(def.kind),
            order: PinOrder(order),
        }
    }

    /// ⚠️ 注意：这里只暴露“当前执行值”，不做优先级判断
    pub fn current_value(&self) -> Option<&DataValue> {
        match &self.runtime {
            PinRuntime::Data(rt) => rt.current.as_ref(),
            PinRuntime::Exec => None,
        }
    }

    /// 用户填写的值（仅在未连接时由 Context 使用）
    pub fn user_value(&self) -> Option<&DataValue> {
        match &self.runtime {
            PinRuntime::Data(rt) => rt.user_override.as_ref(),
            PinRuntime::Exec => None,
        }
    }

    /// 设置运行时值（来自连接 / 节点计算）
    pub fn set_current_value(&mut self, value: DataValue) {
        if let PinRuntime::Data(rt) = &mut self.runtime {
            rt.current = Some(value);
            if let PinState::Data(state) = &mut self.state {
                *state = DataPinState::Ready;
            }
        }
    }

    /// 设置用户值（编辑器行为）
    pub fn set_user_value(&mut self, value: Option<DataValue>) {
        if let PinRuntime::Data(rt) = &mut self.runtime {
            rt.user_override = value;
        }
    }

    /// 清除运行时值（断线 / reset / 重新执行）
    pub fn clear_current_value(&mut self) {
        if let PinRuntime::Data(rt) = &mut self.runtime {
            rt.current = None;
            if let PinState::Data(state) = &mut self.state {
                *state = DataPinState::Uninitialized;
            }
        }
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
