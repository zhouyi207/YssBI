//! 节点的图作用域与系统托管「壳节点」协议
//!
//! - [`NodeGraphScope`] 决定一个节点定义能出现在哪种图里（Event / Function / Any）。
//! - [`ShellRole`] 标记「系统托管壳节点」：随图自动创建、不可删除 / 复制、每图至多一个、
//!   在 palette 中隐藏（用户不能手动添加）。其 pin 是图签名的投影（Phase 2）。
//!
//! 这些语义全部由后端定义并强制执行；前端仅据此做 UX 层面的隐藏与拦截。

use serde::{Deserialize, Serialize};

/// 节点定义允许出现的图类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeGraphScope {
    /// 任意图都可用（绝大多数数据/控制节点）。
    #[default]
    Any,
    /// 仅事件图。
    Event,
    /// 仅函数图。
    Function,
}

impl NodeGraphScope {
    /// 该作用域是否允许在指定图类型中使用。
    pub fn allows(&self, kind: &crate::graph::GraphKind) -> bool {
        match self {
            NodeGraphScope::Any => true,
            NodeGraphScope::Event => matches!(kind, crate::graph::GraphKind::Event),
            NodeGraphScope::Function => matches!(kind, crate::graph::GraphKind::Function),
        }
    }
}

/// 系统托管壳节点的角色。带角色的节点即为「壳」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellRole {
    /// 事件图入口。
    EventBegin,
    /// 函数图入口（投影 function_inputs，Phase 2）。
    FunctionEntry,
    /// 函数图返回（投影 function_outputs，Phase 2）。
    FunctionReturn,
}
