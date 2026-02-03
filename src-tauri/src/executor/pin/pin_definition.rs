//! Pin 定义（静态描述）

use super::{PinGroup, PinRole};
use crate::executor::value::{DataValue, PinTypeDesc};
use serde::{Deserialize, Serialize};

/// Pin 方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDirection {
    Input,
    Output,
}

/// Pin 类型（数据 vs 执行）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinKind {
    /// 数据 Pin
    Data,
    /// 执行 Pin
    Exec,
}

/// Pin 定义（静态描述，用于节点原型）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinDefinition {
    /// Pin 名称（仅用于 UI/Debug）
    pub name: String,
    
    /// Pin 方向
    pub direction: PinDirection,
    
    /// Pin 类型
    pub kind: PinKind,
    
    /// 语义角色（逻辑锚点）
    pub role: PinRole,
    
    /// 分组（可选，用于动态 Pin）
    pub group: Option<PinGroup>,
    
    /// 类型描述（仅 Data Pin）
    pub type_desc: Option<PinTypeDesc>,
    
    /// 默认值（仅 Data Pin）
    pub default_value: Option<DataValue>,
    
    /// 是否显示 Widget
    pub show_widget: bool,
    
    /// Widget 类型
    pub widget_type: Option<String>,
}

impl PinDefinition {
    /// 创建数据输入 Pin
    pub fn data_input(name: impl Into<String>, role: PinRole, type_desc: PinTypeDesc) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Input,
            kind: PinKind::Data,
            role,
            group: None,
            type_desc: Some(type_desc),
            default_value: None,
            show_widget: true,
            widget_type: None,
        }
    }

    /// 创建数据输出 Pin
    pub fn data_output(name: impl Into<String>, role: PinRole, type_desc: PinTypeDesc) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Output,
            kind: PinKind::Data,
            role,
            group: None,
            type_desc: Some(type_desc),
            default_value: None,
            show_widget: false,
            widget_type: None,
        }
    }

    /// 创建执行输入 Pin
    pub fn exec_input(name: impl Into<String>, role: PinRole) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Input,
            kind: PinKind::Exec,
            role,
            group: None,
            type_desc: None,
            default_value: None,
            show_widget: false,
            widget_type: None,
        }
    }

    /// 创建执行输出 Pin
    pub fn exec_output(name: impl Into<String>, role: PinRole) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Output,
            kind: PinKind::Exec,
            role,
            group: None,
            type_desc: None,
            default_value: None,
            show_widget: false,
            widget_type: None,
        }
    }

    /// 设置分组
    pub fn with_group(mut self, group: PinGroup) -> Self {
        self.group = Some(group);
        self
    }

    /// 设置默认值
    pub fn with_default(mut self, value: DataValue) -> Self {
        self.default_value = Some(value);
        self
    }

    /// 设置 Widget
    pub fn with_widget(mut self, widget_type: impl Into<String>) -> Self {
        self.show_widget = true;
        self.widget_type = Some(widget_type.into());
        self
    }
}
