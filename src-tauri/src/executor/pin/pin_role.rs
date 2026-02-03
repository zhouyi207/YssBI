//! Pin 语义角色系统
//!
//! Pin 的逻辑绑定通过语义角色（PinRole）完成，而不是通过名称或索引。
//! 这是 Blueprint 风格节点系统的核心设计。

use serde::{Deserialize, Serialize};

/// Pin 语义角色
///
/// 定义 Pin 在节点逻辑中的语义作用，而不是通过名称或位置。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinRole {
    // ===== 通用角色 =====
    /// 主输入
    PrimaryInput,
    /// 主输出
    PrimaryOutput,
    
    // ===== 控制流角色 =====
    /// 执行输入
    ExecIn,
    /// 执行输出（默认）
    ExecOut,
    /// 条件
    Condition,
    /// True 分支
    TrueBranch,
    /// False 分支
    FalseBranch,
    
    // ===== 数学运算角色 =====
    /// 操作数（支持多个，通过 Group 区分）
    Operand,
    /// 运算结果
    Result,
    
    // ===== 序列/步骤角色 =====
    /// 序列步骤（支持多个）
    Step(u32),
    
    // ===== 变量角色 =====
    /// 变量引用
    VariableRef,
    /// 变量值
    VariableValue,
    
    // ===== 动态角色 =====
    /// 动态 Pin（通过 Group 标识）
    Dynamic(String),
    
    // ===== 自定义角色 =====
    /// 自定义语义
    Custom(String),
}

impl PinRole {
    /// 创建操作数角色
    pub fn operand() -> Self {
        PinRole::Operand
    }

    /// 创建步骤角色
    pub fn step(index: u32) -> Self {
        PinRole::Step(index)
    }

    /// 创建动态角色
    pub fn dynamic(group: impl Into<String>) -> Self {
        PinRole::Dynamic(group.into())
    }

    /// 创建自定义角色
    pub fn custom(name: impl Into<String>) -> Self {
        PinRole::Custom(name.into())
    }
}

/// Pin 分组
///
/// 用于将多个相同角色的 Pin 组织在一起（如 Add 的多个 Operand）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinGroup(pub String);

impl PinGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn operands() -> Self {
        Self("operands".to_string())
    }

    pub fn steps() -> Self {
        Self("steps".to_string())
    }
}

impl From<&str> for PinGroup {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for PinGroup {
    fn from(s: String) -> Self {
        Self(s)
    }
}
