//! Pin 定义（静态描述）
//!
//!
//! 在这里 output 也需要有默认值吗？？？？

use super::PinTypeDesc;
use super::{DataRole, ExecRole, PinRole};
use crate::executor::DataValue;
use serde::{Deserialize, Serialize};

/// Pin 方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinDirection {
    /// 输入
    Input,
    /// 输出
    Output,
}

/// Pin 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinKind {
    /// 数据 Pin
    Data,
    /// 执行 Pin
    Exec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinMetaData {
    /// 是否显示 Widget
    pub show_widget: bool,

    /// Widget 类型
    pub widget_type: Option<String>,

    /// 动态添加
    pub is_dynamic: bool,
}

impl Default for PinMetaData {
    fn default() -> Self {
        Self {
            show_widget: false,
            widget_type: None,
            is_dynamic: false,
        }
    }
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

    /// 类型描述（仅 Data Pin）
    pub type_desc: Option<PinTypeDesc>,

    /// 默认值（仅 Data + Input Pin 有意义）
    pub default_value: Option<DataValue>,

    /// UI / 编辑器相关元数据
    pub meta_data: PinMetaData,
}

impl PinDefinition {
    /// 创建数据输入 Pin
    pub fn data_input(name: impl Into<String>, role: DataRole, type_desc: PinTypeDesc) -> Self {
        let default_value = type_desc.default_value();

        Self {
            name: name.into(),
            direction: PinDirection::Input,
            kind: PinKind::Data,
            role: PinRole::Data(role),
            type_desc: Some(type_desc),
            default_value: default_value,
            meta_data: PinMetaData::default(),
        }
    }

    /// 创建数据输出 Pin
    pub fn data_output(name: impl Into<String>, role: DataRole, type_desc: PinTypeDesc) -> Self {
        let default_value = type_desc.default_value();
        Self {
            name: name.into(),
            direction: PinDirection::Output,
            kind: PinKind::Data,
            role: PinRole::Data(role),
            type_desc: Some(type_desc),
            default_value: default_value,
            meta_data: PinMetaData::default(),
        }
    }

    /// 创建执行输入 Pin
    pub fn exec_input(name: impl Into<String>, role: ExecRole) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Input,
            kind: PinKind::Exec,
            role: PinRole::Exec(role),
            type_desc: None,
            default_value: None,
            meta_data: PinMetaData::default(),
        }
    }

    /// 创建执行输出 Pin
    pub fn exec_output(name: impl Into<String>, role: ExecRole) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Output,
            kind: PinKind::Exec,
            role: PinRole::Exec(role),
            type_desc: None,
            default_value: None,
            meta_data: PinMetaData::default(),
        }
    }

    /// 设置 Widget
    pub fn with_metadata(mut self, widget_type: impl Into<String>) -> Self {
        self.meta_data.show_widget = true;
        self.meta_data.widget_type = Some(widget_type.into());
        self
    }

    /// 动态添加 Pin
    pub fn dynamic(mut self) -> Self {
        self.meta_data.is_dynamic = true;
        self
    }

    pub fn with_default(mut self, default_value: Option<DataValue>) -> Self {
        self.default_value = default_value;
        self
    }

    /// role 兜底校验
    pub fn validate(&self) {
        match (&self.kind, &self.role) {
            (PinKind::Exec, PinRole::Data(_)) => panic!("Exec pin cannot have Data role"),
            (PinKind::Data, PinRole::Exec(_)) => panic!("Data pin cannot have Exec role"),
            _ => {}
        }
    }
}
