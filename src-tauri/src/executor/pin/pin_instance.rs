//! Pin 实例（运行时）
//!
//! Pin 实例由 Graph 管理，不属于 Node。
//! Pin 不存储连接信息，所有连接由 ConnectionManager 管理。

use super::{DataPinState, ExecPinState, PinDefinition, PinDirection, PinId, PinKind, PinRole};
use crate::executor::value::{DataValue, PinTypeDesc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Node 标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for NodeId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<NodeId> for Uuid {
    fn from(id: NodeId) -> Self {
        id.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Pin 实例（运行时）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    /// Pin ID
    pub id: PinId,
    
    /// 所属节点 ID
    pub node_id: NodeId,
    
    /// Pin 名称（仅用于 UI/Debug）
    pub name: String,
    
    /// Pin 方向
    pub direction: PinDirection,
    
    /// Pin 类型
    pub kind: PinKind,
    
    /// 语义角色
    pub role: PinRole,
    
    /// 类型描述（仅 Data Pin）
    pub type_desc: Option<PinTypeDesc>,
    
    /// 当前值（仅 Data Pin）
    pub value: Option<DataValue>,
    
    /// 用户设置的值（仅 Data Pin）
    pub user_value: Option<DataValue>,
    
    /// 数据 Pin 状态
    pub data_state: Option<DataPinState>,
    
    /// 执行 Pin 状态
    pub exec_state: Option<ExecPinState>,
    
    /// 是否显示 Widget
    pub show_widget: bool,
    
    /// Widget 类型
    pub widget_type: Option<String>,
}

impl PinInstance {
    /// 从定义创建实例
    pub fn from_definition(def: &PinDefinition, node_id: NodeId) -> Self {
        let id = PinId::new();
        
        let (data_state, exec_state) = match def.kind {
            PinKind::Data => (Some(DataPinState::default()), None),
            PinKind::Exec => (None, Some(ExecPinState::default())),
        };

        Self {
            id,
            node_id,
            name: def.name.clone(),
            direction: def.direction,
            kind: def.kind,
            role: def.role.clone(),
            type_desc: def.type_desc.clone(),
            value: def.default_value.clone(),
            user_value: None,
            data_state,
            exec_state,
            show_widget: def.show_widget,
            widget_type: def.widget_type.clone(),
        }
    }

    /// 获取有效值（优先用户值，其次当前值）
    pub fn effective_value(&self) -> Option<&DataValue> {
        self.user_value.as_ref().or(self.value.as_ref())
    }

    /// 设置值
    pub fn set_value(&mut self, value: DataValue) {
        self.value = Some(value);
        if let Some(state) = &mut self.data_state {
            *state = DataPinState::Ready;
        }
    }

    /// 设置用户值
    pub fn set_user_value(&mut self, value: Option<DataValue>) {
        self.user_value = value;
    }

    /// 清除值
    pub fn clear_value(&mut self) {
        self.value = None;
        if let Some(state) = &mut self.data_state {
            *state = DataPinState::Uninitialized;
        }
    }

    /// 是否为数据 Pin
    pub fn is_data(&self) -> bool {
        matches!(self.kind, PinKind::Data)
    }

    /// 是否为执行 Pin
    pub fn is_exec(&self) -> bool {
        matches!(self.kind, PinKind::Exec)
    }

    /// 是否为输入 Pin
    pub fn is_input(&self) -> bool {
        matches!(self.direction, PinDirection::Input)
    }

    /// 是否为输出 Pin
    pub fn is_output(&self) -> bool {
        matches!(self.direction, PinDirection::Output)
    }
}
