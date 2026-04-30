//! Pin 定义（静态描述）
//! PinDefinition = 规则声明（Schema / Contract）
//!
//! 因此不能将实例或者运行时的状态如 value 带入

use crate::graph::value::DataValue;
use crate::graph::TypeVarKey;

use super::{DataRole, ExecRole, PinDataTypeDefinition, PinRole};
use serde::{Deserialize, Serialize};

/// Pin 方向（lowercase 序列化以与前端 "input"|"output" 一致）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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
#[serde(rename_all = "camelCase")]
pub struct PinMetaData {
    /// 是否显示 Widget
    pub show_widget: bool,

    /// Widget 类型
    pub widget_type: Option<String>,

    /// 动态添加
    pub is_dynamic: bool,

    /// Widget 可选项（用于 dropdown 等控件）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_options: Option<Vec<String>>,
}

impl Default for PinMetaData {
    fn default() -> Self {
        Self {
            show_widget: false,
            widget_type: None,
            is_dynamic: false,
            widget_options: None,
        }
    }
}

/// Pin 定义（静态描述，用于节点原型）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub data_type: Option<PinDataTypeDefinition>,

    /// 是否可选（true = 无需连接也可运行，false = 必须连接）
    #[serde(default)]
    pub optional: bool,

    /// 自定义默认值（覆盖类型的 default_value，用于 pin 初始显示和运行时回退）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<DataValue>,

    /// UI / 编辑器相关元数据
    pub meta_data: PinMetaData,
}

impl PinDefinition {
    /// 创建数据输入 Pin
    pub fn data_input(
        name: impl Into<String>,
        role: DataRole,
        data_type: PinDataTypeDefinition,
    ) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Input,
            kind: PinKind::Data,
            role: PinRole::Data(role),
            data_type: Some(data_type),
            optional: false,
            default_value: None,
            meta_data: PinMetaData::default(),
        }
    }

    /// 创建数据输出 Pin
    pub fn data_output(
        name: impl Into<String>,
        role: DataRole,
        data_type: PinDataTypeDefinition,
    ) -> Self {
        Self {
            name: name.into(),
            direction: PinDirection::Output,
            kind: PinKind::Data,
            role: PinRole::Data(role),
            data_type: Some(data_type),
            optional: false,
            default_value: None,
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
            optional: false,
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
            data_type: None,
            optional: false,
            default_value: None,
            meta_data: PinMetaData::default(),
        }
    }

    /// 标记为可选（无需连接也可运行）
    pub fn with_optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// 设置自定义默认值（覆盖类型的 default_value）
    pub fn with_default_value(mut self, value: DataValue) -> Self {
        self.default_value = Some(value);
        self
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

    /// 设置 Widget 可选项（用于 dropdown 等控件）
    pub fn with_widget_options(mut self, options: Vec<String>) -> Self {
        self.meta_data.widget_options = Some(options);
        self
    }

    pub fn get_type_var_key(&self) -> Option<TypeVarKey> {
        // Extract TypeVarKey from PinDataTypeDefinition if it's a TypeVar
        if let Some(data_type) = &self.data_type {
            if let crate::graph::pin::PinDataTypeDefinition::TypeVar(key) = data_type {
                return Some(key.clone());
            }
        }
        None
    }

    pub fn is_data(&self) -> bool {
        matches!(self.kind, PinKind::Data)
    }

    pub fn is_exec(&self) -> bool {
        matches!(self.kind, PinKind::Exec)
    }

    pub fn is_input(&self) -> bool {
        matches!(self.direction, PinDirection::Input)
    }

    pub fn is_output(&self) -> bool {
        matches!(self.direction, PinDirection::Output)
    }
}
