//! Pin 语义角色系统
//!
//! Pin 的逻辑绑定通过语义角色（PinRole）完成，而不是通过名称或索引。
//! 这是 Blueprint 风格节点系统的核心设计。

use serde::{Deserialize, Serialize};

/// 控制流角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecRole {
    /// 主执行输入
    ExecIn,
    /// 主执行输出
    ExecOut,
    /// 条件分支 - True 路径
    ExecTrue,
    /// 条件分支 - False 路径
    ExecFalse,
    /// 循环体执行
    ExecLoopBody,
    /// 循环完成
    ExecLoopComplete,
    /// 序列步骤（如 Sequence 的多个执行输出）
    Steps(usize),
    /// 分支情况（如 Switch 的多个分支）
    Cases,
    /// 自定义语义角色
    Custom(String),
}

/// 数据流角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataRole {
    /// 条件值
    Condition,
    // 操作值
    Operands(usize),
    /// 主输入值
    Input,
    Inputs(usize),
    /// 主输出值
    Output,
    Outputs(usize),
    /// 结果值
    Result,
    /// 错误信息
    Error,
    /// 自定义语义角色
    Custom(String),
}

/// Pin 语义角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PinRole {
    /// 执行角色
    Exec(ExecRole),
    /// 数据角色
    Data(DataRole),
}

impl PinRole {
    /// 是否是 exec pin role
    pub fn is_exec(&self) -> bool {
        matches!(self, PinRole::Exec(_))
    }

    /// 是否是 data pin role
    pub fn is_data(&self) -> bool {
        matches!(self, PinRole::Data(_))
    }

    /// 获取 role 的 family name
    pub fn family(&self) -> &str {
        match self {
            PinRole::Exec(item) => match item {
                ExecRole::ExecIn => "exec.in",
                ExecRole::ExecOut => "exec.out",
                ExecRole::ExecTrue => "exec.true",
                ExecRole::ExecFalse => "exec.false",
                ExecRole::ExecLoopBody => "exec.loop.body",
                ExecRole::ExecLoopComplete => "exec.loop.complete",
                ExecRole::Steps(_) => "exec.steps",
                ExecRole::Cases => "exec.cases",
                ExecRole::Custom(name) => name.as_str(),
            },
            PinRole::Data(item) => match item {
                DataRole::Condition => "data.condition",
                DataRole::Operands(_) => "data.operands",
                DataRole::Input => "data.in",
                DataRole::Inputs(_) => "data.ins",
                DataRole::Output => "data.out",
                DataRole::Outputs(_) => "data.outs",
                DataRole::Result => "data.result",
                DataRole::Error => "data.error",
                DataRole::Custom(name) => name.as_str(),
            },
        }
    }

    /// 获取 role 的 index，仅对带 index 的 role 有效
    pub fn index(&self) -> Option<usize> {
        match self {
            PinRole::Data(DataRole::Inputs(i) | DataRole::Outputs(i) | DataRole::Operands(i)) => Some(*i),
            PinRole::Exec(ExecRole::Steps(i)) => Some(*i),
            _ => None,
        }
    }

    /// 检查两个角色是否属于同一家族
    pub fn is_same_family(&self, other: &PinRole) -> bool {
        self.family() == other.family()
    }

    /// 创建同家族但不同索引的角色
    ///
    /// 仅对可索引的角色有效（Operands, Inputs, Outputs, Steps）
    pub fn with_index(&self, index: usize) -> Option<PinRole> {
        match self {
            PinRole::Data(DataRole::Operands(_)) => Some(PinRole::Data(DataRole::Operands(index))),
            PinRole::Data(DataRole::Inputs(_)) => Some(PinRole::Data(DataRole::Inputs(index))),
            PinRole::Data(DataRole::Outputs(_)) => Some(PinRole::Data(DataRole::Outputs(index))),
            PinRole::Exec(ExecRole::Steps(_)) => Some(PinRole::Exec(ExecRole::Steps(index))),
            _ => None,
        }
    }

    /// 检查角色是否匹配指定的家族模式
    /// 例如：Operands(1) 和 Operands(2) 都匹配 Operands(_)
    pub fn matches_family(&self, pattern: &PinRole) -> bool {
        match (self, pattern) {
            // 数据角色家族匹配
            (PinRole::Data(DataRole::Operands(_)), PinRole::Data(DataRole::Operands(_))) => true,
            (PinRole::Data(DataRole::Inputs(_)), PinRole::Data(DataRole::Inputs(_))) => true,
            (PinRole::Data(DataRole::Outputs(_)), PinRole::Data(DataRole::Outputs(_))) => true,
            (PinRole::Exec(ExecRole::Steps(_)), PinRole::Exec(ExecRole::Steps(_))) => true,
            // 精确匹配
            (a, b) => a == b,
        }
    }
}
