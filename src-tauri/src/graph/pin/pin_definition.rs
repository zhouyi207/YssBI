//! Pin 定义（静态描述）
//! PinDefinition = 规则声明（Schema / Contract）
//!
//! 因此不能将实例或者运行时的状态如 value 带入

use super::{DataRole, ExecRole, PinDataType, PinRole};
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
    /// Pin 名称
    pub name: String,

    /// Pin 方向
    pub direction: PinDirection,

    /// Pin 类型
    pub kind: PinKind,

    /// 语义角色（逻辑锚点）
    pub role: PinRole,

    /// 类型描述（仅 Data Pin）
    pub data_type: Option<PinDataType>,

    /// UI / 编辑器相关元数据
    pub meta_data: PinMetaData,
}

impl PinDefinition {
    /// 创建数据输入 Pin
    pub fn data_input(name: impl Into<String>, role: DataRole, data_type: PinDataType) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Input,
            kind: PinKind::Data,
            role: PinRole::Data(role),
            data_type: Some(data_type),
            meta_data: PinMetaData::default(),
        }
    }

    /// 创建数据输出 Pin
    pub fn data_output(name: impl Into<String>, role: DataRole, data_type: PinDataType) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Output,
            kind: PinKind::Data,
            role: PinRole::Data(role),
            data_type: Some(data_type),
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
            data_type: None,
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
            data_type: None,
            meta_data: PinMetaData::default(),
        }
    }

    /// 设置 Widget
    pub fn with_metadata(mut self, show_widget: bool, widget_type: impl Into<String>) -> Self {
        self.meta_data.show_widget = show_widget;
        self.meta_data.widget_type = Some(widget_type.into());
        self
    }

    /// 动态添加 Pin
    pub fn with_dynamic(mut self, is_dynamic: bool) -> Self {
        self.meta_data.is_dynamic = is_dynamic;
        self
    }
}
