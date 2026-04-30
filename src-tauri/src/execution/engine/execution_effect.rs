//! 执行效果（Execution Effect）
//!
//! 节点不返回"下一跳"，而是返回"执行效果"
//! 执行器负责解释效果并更新 continuation 栈

use crate::graph::pin::ExecRole;
use serde::{Deserialize, Serialize};

/// 恢复令牌（用于 Delay / Async）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResumeToken(pub String);

/// 节点执行效果
///
/// 这是节点执行后产生的"效果声明"，而不是"下一跳指令"
/// 执行器负责解释这些效果并决定如何继续执行
#[derive(Debug, Clone)]
pub enum ExecutionEffect {
    /// 完成执行，没有后续控制流
    ///
    /// 适用于：
    /// - 纯数据节点（Add, Multiply）
    /// - 没有输出 exec pin 的节点
    Done,

    /// 触发单个输出 Exec Pin
    ///
    /// 适用于：
    /// - 简单控制流节点（Branch 的 True/False）
    /// - 只有一个输出的节点
    ///
    /// 执行器会：
    /// 1. 查找该输出连接的下游节点
    /// 2. 将下游节点压入执行栈
    TriggerOutput(ExecRole),

    /// 触发输出并等待子流程完成后继续
    ///
    /// 适用于：
    /// - Sequence 节点
    /// - 需要按顺序执行多个输出的节点
    ///
    /// 执行器会：
    /// 1. 保存当前帧（标记为 WaitingForChild）
    /// 2. 将 remaining 保存为 continuation
    /// 3. 触发 current 输出
    /// 4. 当 current 的子流程完成后，继续执行 remaining
    TriggerAndContinue {
        /// 当前要触发的输出
        current: ExecRole,
        /// 剩余要执行的输出（按顺序）
        remaining: Vec<ExecRole>,
    },

    /// 触发多个输出（并行或顺序，由执行器决定）
    ///
    /// 适用于：
    /// - 需要触发多个输出但不关心顺序的节点
    ///
    /// 注意：当前实现会按顺序执行（逆序压栈）
    TriggerSequence(Vec<ExecRole>),

    /// 暂停执行，等待外部事件
    ///
    /// 适用于：
    /// - Delay 节点
    /// - Async / Await 节点
    /// - 等待用户输入的节点
    ///
    /// 执行器会：
    /// 1. 将当前帧保存到挂起队列
    /// 2. 继续执行栈中的其他帧
    /// 3. 当外部事件触发时，通过 resume_token 恢复执行
    Suspend {
        resume_token: ResumeToken,
        /// 恢复时要触发的输出
        resume_output: ExecRole,
    },

    /// 循环执行
    ///
    /// 适用于：
    /// - Loop 节点
    /// - While 节点
    /// - For 节点
    ///
    /// 执行器会：
    /// 1. 触发 body 输出
    /// 2. 当 body 完成后，重新评估条件
    /// 3. 如果条件为真，重复步骤 1
    /// 4. 如果条件为假，触发 completed 输出
    Loop {
        /// 循环体输出
        body: ExecRole,
        /// 循环完成输出
        completed: ExecRole,
        /// 是否继续循环（由节点状态决定）
        should_continue: bool,
    },
}

impl ExecutionEffect {
    /// 创建简单的完成效果
    pub fn done() -> Self {
        ExecutionEffect::Done
    }

    /// 创建触发单个输出的效果
    pub fn trigger(role: ExecRole) -> Self {
        ExecutionEffect::TriggerOutput(role)
    }

    /// 创建 Sequence 效果（触发并继续）
    pub fn sequence(roles: Vec<ExecRole>) -> Self {
        if roles.is_empty() {
            return ExecutionEffect::Done;
        }

        if roles.len() == 1 {
            return ExecutionEffect::TriggerOutput(roles[0].clone());
        }

        ExecutionEffect::TriggerAndContinue {
            current: roles[0].clone(),
            remaining: roles[1..].to_vec(),
        }
    }

    /// 创建暂停效果
    pub fn suspend(token: impl Into<String>, resume_output: ExecRole) -> Self {
        ExecutionEffect::Suspend {
            resume_token: ResumeToken(token.into()),
            resume_output,
        }
    }

    /// 创建循环效果
    pub fn loop_effect(body: ExecRole, completed: ExecRole, should_continue: bool) -> Self {
        ExecutionEffect::Loop {
            body,
            completed,
            should_continue,
        }
    }
}
