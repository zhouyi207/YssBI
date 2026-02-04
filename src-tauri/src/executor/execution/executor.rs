//! 执行器（Executor）
//!
//! 执行器是唯一负责调度执行顺序的组件
//! 它解释节点返回的 ExecutionEffect 并管理 continuation 栈

use super::execution_effect::{ExecutionEffect, ResumeToken};
use super::execution_frame::{ExecutionFrame, FrameId, FrameState};
use super::execution_stack::ExecutionStack;
use crate::executor::graph::Graph;
use crate::executor::node::{NodeExecutionContext, NodeId};
use crate::executor::pin::{ExecRole, PinRole};
use crate::executor::value::DataValue;
use std::collections::HashMap;
use std::sync::Arc;

/// 执行器
///
/// 负责：
/// - 解释 ExecutionEffect
/// - 管理执行栈
/// - 调度节点执行
/// - 处理暂停/恢复
pub struct Executor {
    /// 执行栈
    stack: ExecutionStack,

    /// 暂停的帧（按 ResumeToken 索引）
    suspended_frames: HashMap<ResumeToken, FrameId>,

    /// Graph 引用
    graph: Arc<Graph>,

    /// 执行日志
    logs: Vec<String>,
}

impl Executor {
    /// 创建新的执行器
    pub fn new(graph: Arc<Graph>) -> Self {
        Self {
            stack: ExecutionStack::new(),
            suspended_frames: HashMap::new(),
            graph,
            logs: Vec::new(),
        }
    }

    /// 开始执行（从指定节点开始）
    pub fn start(&mut self, entry_node: NodeId) -> Result<(), String> {
        self.log(format!("Starting execution from node {:?}", entry_node));

        // 创建根帧
        let frame_id = self.stack.push_new(entry_node, None, None);
        self.log(format!("Created root frame {:?}", frame_id));

        // 执行主循环
        self.run()
    }

    /// 主执行循环
    ///
    /// 核心算法：
    /// 1. 从栈顶取出一帧
    /// 2. 执行该帧的节点
    /// 3. 根据返回的 ExecutionEffect 更新栈
    /// 4. 重复直到栈为空
    fn run(&mut self) -> Result<(), String> {
        while !self.stack.is_empty() {
            // 取出栈顶帧
            let mut frame = self.stack.pop().ok_or("Stack is empty")?;
            self.log(format!("Executing frame {:?}", frame.id));

            // 更新帧状态
            frame.state = FrameState::Running;

            // 执行节点
            let effect = self.execute_node(&frame)?;
            self.log(format!("Node returned effect: {:?}", effect));

            // 解释执行效果
            self.interpret_effect(effect, frame)?;
        }

        self.log("Execution completed".to_string());
        Ok(())
    }

    /// 执行节点并返回 ExecutionEffect
    fn execute_node(&mut self, frame: &ExecutionFrame) -> Result<ExecutionEffect, String> {
        let node_id = frame.node_id;

        // 获取节点定义
        let definition = self
            .graph
            .get_node_definition(node_id)
            .ok_or_else(|| format!("Node {:?} not found", node_id))?;

        // 创建执行上下文
        let mut ctx = GraphNodeExecutionContext::new(node_id, self.graph.clone());

        // 执行节点的 FlowProcessor（如果有）
        if let Some(ref processor) = definition.flow_processor {
            return processor(&mut ctx);
        }

        // 执行节点的 DataEvaluator（如果有）
        if let Some(data_evaluator) = &definition.data_evaluator {
            data_evaluator(&mut ctx)?;
        }

        // 如果没有 FlowProcessor，返回 Done
        Ok(ExecutionEffect::Done)
    }

    /// 解释执行效果并更新栈
    fn interpret_effect(
        &mut self,
        effect: ExecutionEffect,
        frame: ExecutionFrame,
    ) -> Result<(), String> {
        match effect {
            ExecutionEffect::Done => {
                self.log(format!("Frame {:?} completed", frame.id));
                self.handle_frame_completion(frame)?;
            }

            ExecutionEffect::TriggerOutput(role) => {
                self.log(format!("Triggering output {:?}", role));
                self.trigger_output(frame, role)?;
            }

            ExecutionEffect::TriggerAndContinue { current, remaining } => {
                self.log(format!(
                    "Triggering {:?} with {} remaining",
                    current,
                    remaining.len()
                ));
                self.trigger_and_continue(frame, current, remaining)?;
            }

            ExecutionEffect::TriggerSequence(roles) => {
                self.log(format!("Triggering sequence of {} outputs", roles.len()));
                self.trigger_sequence(frame, roles)?;
            }

            ExecutionEffect::Suspend {
                resume_token,
                resume_output,
            } => {
                self.log(format!("Suspending frame {:?}", frame.id));
                self.suspend_frame(frame, resume_token, resume_output)?;
            }

            ExecutionEffect::Loop {
                body,
                completed,
                should_continue,
            } => {
                self.log(format!("Loop: should_continue={}", should_continue));
                self.handle_loop(frame, body, completed, should_continue)?;
            }
        }

        Ok(())
    }

    /// 处理帧完成
    ///
    /// 当一个帧完成时：
    /// 1. 检查是否有父帧
    /// 2. 如果有父帧且父帧在等待子流程完成，恢复父帧
    fn handle_frame_completion(&mut self, frame: ExecutionFrame) -> Result<(), String> {
        if let Some(parent_id) = frame.parent_frame {
            self.log(format!("Frame {:?} has parent {:?}", frame.id, parent_id));

            // 检查父帧是否还有其他子流程在执行
            if !self.stack.has_active_children(parent_id) {
                self.log(format!("All children of {:?} completed", parent_id));

                // 获取父帧信息
                let (parent_node, next_role, should_resume) = {
                    let parent_frame = self.stack.get_frame_mut(parent_id);
                    if let Some(parent_frame) = parent_frame {
                        if parent_frame.state == FrameState::WaitingForChild {
                            parent_frame.state = FrameState::Ready;

                            // 如果父帧有剩余的 continuation，获取下一个
                            let next_role = parent_frame.pop_continuation();
                            (parent_frame.node_id, next_role, true)
                        } else {
                            (parent_frame.node_id, None, false)
                        }
                    } else {
                        return Ok(());
                    }
                };

                // 如果需要恢复且有下一个 continuation，触发它
                if should_resume {
                    self.log(format!("Resuming parent frame {:?}", parent_id));
                    if let Some(next_role) = next_role {
                        self.log(format!("Parent has continuation: {:?}", next_role));
                        self.trigger_output_from_node(parent_node, next_role, Some(parent_id))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// 触发单个输出
    fn trigger_output(&mut self, frame: ExecutionFrame, role: ExecRole) -> Result<(), String> {
        self.trigger_output_from_node(frame.node_id, role, Some(frame.id))
    }

    /// 从指定节点触发输出
    fn trigger_output_from_node(
        &mut self,
        node_id: NodeId,
        role: ExecRole,
        parent_frame: Option<FrameId>,
    ) -> Result<(), String> {
        self.trigger_output_from_node_and_check(node_id, role, parent_frame)?;
        Ok(())
    }

    /// 触发并继续（用于 Sequence）
    fn trigger_and_continue(
        &mut self,
        mut frame: ExecutionFrame,
        current: ExecRole,
        mut remaining: Vec<ExecRole>,
    ) -> Result<(), String> {
        let frame_id = frame.id;
        let node_id = frame.node_id;

        // 循环处理所有没有下游连接的输出
        let mut current_role = current;
        loop {
            // 触发当前输出
            let has_downstream = self.trigger_output_from_node_and_check(
                node_id,
                current_role.clone(),
                Some(frame_id)
            )?;

            if has_downstream {
                // 有下游连接：保存剩余的 continuation，将帧压回栈等待子流程完成
                frame.remaining_continuations = remaining;
                frame.state = FrameState::WaitingForChild;
                self.stack.push(frame);
                return Ok(());
            }

            // 没有下游连接：立即处理下一个 continuation
            self.log(format!(
                "No downstream for {:?}, processing next continuation immediately",
                current_role
            ));

            if remaining.is_empty() {
                // 没有剩余的 continuation，帧完成
                self.log(format!("Frame {:?} completed (no more continuations)", frame_id));
                self.handle_frame_completion(frame)?;
                return Ok(());
            }

            // 取出下一个 continuation 并继续循环
            current_role = remaining.remove(0);
        }
    }

    /// 触发输出并返回是否有下游连接
    fn trigger_output_from_node_and_check(
        &mut self,
        node_id: NodeId,
        role: ExecRole,
        parent_frame: Option<FrameId>,
    ) -> Result<bool, String> {
        // 查找输出 Pin
        let pin_role = PinRole::Exec(role);
        let output_pin = self
            .graph
            .get_pin_by_role(node_id, &pin_role)
            .ok_or_else(|| format!("Output pin {:?} not found on node {:?}", pin_role, node_id))?;

        // 查找下游连接
        let downstream_pins = self.graph.connections().get_downstream(output_pin.id);

        if downstream_pins.is_empty() {
            self.log(format!("No downstream connections for {:?}", pin_role));
            return Ok(false);
        }

        // 为每个下游节点创建新帧
        for &downstream_pin_id in downstream_pins.iter() {
            let downstream_node = self
                .graph
                .connections()
                .get_pin_node(downstream_pin_id)
                .ok_or_else(|| format!("Node for pin {:?} not found", downstream_pin_id))?;

            self.log(format!(
                "Creating frame for downstream node {:?}",
                downstream_node
            ));

            let frame_id = self
                .stack
                .push_new(downstream_node, Some(downstream_pin_id), parent_frame);

            self.log(format!("Created frame {:?}", frame_id));
        }

        Ok(true)
    }

    /// 触发序列（逆序压栈）
    fn trigger_sequence(&mut self, frame: ExecutionFrame, roles: Vec<ExecRole>) -> Result<(), String> {
        // 逆序压栈，使得第一个输出最后执行
        for role in roles.into_iter().rev() {
            self.trigger_output(frame.clone(), role)?;
        }
        Ok(())
    }

    /// 暂停帧
    fn suspend_frame(
        &mut self,
        mut frame: ExecutionFrame,
        resume_token: ResumeToken,
        _resume_output: ExecRole,
    ) -> Result<(), String> {
        frame.state = FrameState::Suspended;
        let frame_id = frame.id;

        // 保存帧到暂停队列
        self.suspended_frames.insert(resume_token, frame_id);

        // 注意：不将帧压回栈，它会在恢复时重新压入
        Ok(())
    }

    /// 恢复暂停的帧
    pub fn resume(&mut self, token: ResumeToken) -> Result<(), String> {
        let frame_id = self
            .suspended_frames
            .remove(&token)
            .ok_or_else(|| format!("Resume token {:?} not found", token))?;

        // 将帧重新压入栈
        // TODO: 需要从某处恢复帧的状态
        // 这里需要一个帧存储机制

        self.log(format!("Resumed frame {:?}", frame_id));
        Ok(())
    }

    /// 处理循环
    fn handle_loop(
        &mut self,
        frame: ExecutionFrame,
        body: ExecRole,
        completed: ExecRole,
        should_continue: bool,
    ) -> Result<(), String> {
        if should_continue {
            // 继续循环：触发 body
            self.trigger_output(frame, body)?;
        } else {
            // 循环完成：触发 completed
            self.trigger_output(frame, completed)?;
        }
        Ok(())
    }

    /// 记录日志
    fn log(&mut self, message: String) {
        self.logs.push(message);
    }

    /// 获取执行日志
    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    /// 获取栈的调试信息
    pub fn debug_stack(&self) -> String {
        self.stack.debug_info()
    }
}

/// Graph 节点执行上下文实现
struct GraphNodeExecutionContext {
    node_id: NodeId,
    graph: Arc<Graph>,
    outputs: HashMap<PinRole, DataValue>,
}

impl GraphNodeExecutionContext {
    fn new(node_id: NodeId, graph: Arc<Graph>) -> Self {
        Self {
            node_id,
            graph,
            outputs: HashMap::new(),
        }
    }
}

impl NodeExecutionContext for GraphNodeExecutionContext {
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String> {
        let pin = self
            .graph
            .get_pin_by_role(self.node_id, role)
            .ok_or_else(|| format!("Pin {:?} not found", role))?;

        self.graph
            .resolve_pin_value(pin.id)
            .ok_or_else(|| format!("No value for pin {:?}", role))
    }

    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String> {
        let pins = self.graph.get_pins_by_role(self.node_id, role);
        pins.into_iter()
            .map(|pin| {
                self.graph
                    .resolve_pin_value(pin.id)
                    .ok_or_else(|| format!("No value for pin {:?}", pin.id))
            })
            .collect()
    }

    fn get_inputs_by_family(&self, pattern: &PinRole) -> Result<Vec<DataValue>, String> {
        let pins = self.graph.get_pins_by_role_family(self.node_id, pattern);
        pins.into_iter()
            .map(|pin| {
                self.graph
                    .resolve_pin_value(pin.id)
                    .ok_or_else(|| format!("No value for pin {:?}", pin.id))
            })
            .collect()
    }

    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String> {
        self.outputs.insert(role.clone(), value.clone());

        let pin = self
            .graph
            .get_pin_by_role(self.node_id, role)
            .ok_or_else(|| format!("Pin {:?} not found", role))?;

        self.graph.set_pin_current_value(pin.id, value)
    }

    fn emit_outputs_by_role(
        &mut self,
        role: &PinRole,
        values: Vec<DataValue>,
    ) -> Result<(), String> {
        let pins = self.graph.get_pins_by_role(self.node_id, role);

        if pins.len() != values.len() {
            return Err(format!(
                "Pin count mismatch: {} pins, {} values",
                pins.len(),
                values.len()
            ));
        }

        for (pin, value) in pins.iter().zip(values.iter()) {
            self.graph.set_pin_current_value(pin.id, value.clone())?;
        }

        Ok(())
    }

    fn is_input_connected(&self, role: &PinRole) -> bool {
        if let Some(pin) = self.graph.get_pin_by_role(self.node_id, role) {
            self.graph.connections().get_upstream(pin.id).is_some()
        } else {
            false
        }
    }

    fn node_id(&self) -> NodeId {
        self.node_id
    }

    fn log(&mut self, message: String) {
        println!("[Node {:?}] {}", self.node_id, message);
    }

    fn error(&mut self, message: String) {
        eprintln!("[Node {:?}] ERROR: {}", self.node_id, message);
    }
}
