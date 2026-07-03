//! 执行器（Executor）
//!
//! 执行器是唯一负责调度执行顺序的组件
//! 它解释节点返回的 ExecutionEffect 并管理 continuation 栈

use super::event_emitter::EventEmitter;
use super::execution_effect::{ExecutionEffect, ResumeToken};
use super::execution_event::ExecutionEvent;
use super::execution_frame::{ExecutionFrame, FrameId, FrameState};
use super::execution_stack::ExecutionStack;
use crate::execution::{
    NodeExecutionContext, ResultSourceStore, SourceAction, build_json_presentation_source,
};
use crate::graph::GraphRuntime;
use crate::graph::node::NodeId;
use crate::graph::pin::{ExecRole, PinRole};
use crate::log_exec;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 执行器
///
/// 负责：
/// - 解释 ExecutionEffect
/// - 管理执行栈
/// - 调度节点执行
/// - 处理暂停/恢复
/// - 通过 EventEmitter 流式发送执行事件给前端
pub struct Executor<E: EventEmitter> {
    /// 执行栈
    stack: ExecutionStack,

    /// 暂停的帧（按 ResumeToken 索引）
    suspended_frames: HashMap<ResumeToken, FrameId>,

    /// Graph 引用
    graph: Arc<Mutex<GraphRuntime>>,

    /// 执行日志
    logs: Vec<String>,

    /// 事件发送器
    emitter: E,

    /// 结果 source 存储（窗口结果与运行时 pin 结果共用）
    result_source_store: ResultSourceStore,

    run_id: String,
}

impl<E: EventEmitter> Executor<E> {
    /// 创建新的执行器
    pub fn new(
        graph: Arc<Mutex<GraphRuntime>>,
        emitter: E,
        result_source_store: ResultSourceStore,
    ) -> Self {
        Self {
            stack: ExecutionStack::new(),
            suspended_frames: HashMap::new(),
            graph,
            logs: Vec::new(),
            emitter,
            result_source_store,
            run_id: uuid::Uuid::new_v4().simple().to_string(),
        }
    }

    /// 发送执行事件
    fn emit(&self, event: ExecutionEvent) {
        self.emitter.emit(event);
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

    /// 主执行循环（错误容错）
    fn run(&mut self) -> Result<(), String> {
        let graph_id = { self.graph.lock().unwrap().graph_id().to_string() };
        self.result_source_store.clear_runtime_graph(&graph_id);
        self.emit(ExecutionEvent::ExecutionStart);
        let mut has_error = false;

        while !self.stack.is_empty() {
            let mut frame = self.stack.pop().ok_or("Stack is empty")?;
            self.log(format!("Executing frame {:?}", frame.id));
            frame.state = FrameState::Running;

            let node_id_str = frame.node_id.to_string();

            self.emit(ExecutionEvent::NodeStart {
                node_id: node_id_str.clone(),
            });
            self.emit_data_input_connections(frame.node_id);

            match self.execute_node(&frame) {
                Ok((effect, duration_ms)) => {
                    self.log(format!("Node returned effect: {:?}", effect));
                    self.emit(ExecutionEvent::NodeComplete {
                        node_id: node_id_str,
                        duration_ms,
                    });
                    self.interpret_effect(effect, frame)?;
                }
                Err((err, duration_ms)) => {
                    has_error = true;
                    self.log(format!("Node {:?} failed: {}", frame.node_id, err));
                    log_exec!(
                        crate::log::LogLevel::Error,
                        "Node {} failed: {}",
                        node_id_str,
                        err
                    );
                    self.emit(ExecutionEvent::NodeError {
                        node_id: node_id_str,
                        error: err,
                        duration_ms,
                    });
                    self.handle_frame_completion(frame)?;
                }
            }
        }

        self.log("Execution completed".to_string());
        self.emit(ExecutionEvent::ExecutionComplete { has_error });
        Ok(())
    }

    /// 执行节点并返回 (ExecutionEffect, duration_ms)
    fn execute_node(
        &mut self,
        frame: &ExecutionFrame,
    ) -> Result<(ExecutionEffect, u64), (String, u64)> {
        let node_id = frame.node_id;

        // 获取节点定义
        let definition = {
            let graph = self.graph.lock().unwrap();
            graph.get_node_definition_by_node_id(node_id)
        };

        // 在执行节点之前，先执行所有上游的纯数据节点
        self.execute_upstream_data_nodes(node_id)
            .map_err(|e| (e, 0u64))?;

        // 创建执行上下文
        let mut ctx = NodeExecutionContext::with_result_sources(
            self.graph.clone(),
            node_id,
            self.result_source_store.clone(),
            self.run_id.clone(),
        );

        // 测量主节点计算耗时（用于性能分析）
        let node_start = Instant::now();

        // 执行节点的 FlowProcessor（如果有）
        let result = if let Some(ref processor) = definition.flow_processor {
            processor(&mut ctx).map_err(|e| (e, node_start.elapsed().as_millis() as u64))
        } else {
            // 执行节点的 DataEvaluator（如果有）
            if let Some(data_evaluator) = &definition.data_evaluator {
                data_evaluator(&mut ctx)
                    .map_err(|e| (e, node_start.elapsed().as_millis() as u64))?;
            }
            // 如果没有 FlowProcessor，返回 Done
            Ok(ExecutionEffect::Done)
        };

        // 收集日志
        self.logs.extend(ctx.logs);
        for event in ctx.pin_result_events {
            self.emit(event);
        }

        let node_duration_ms = node_start.elapsed().as_millis() as u64;

        for action in ctx.source_actions {
            match action {
                SourceAction::OpenExisting(source_id) => {
                    if let Some(descriptor) = self.result_source_store.get_descriptor(&source_id) {
                        self.emit(ExecutionEvent::OpenSourceWindow {
                            source_id,
                            presentation: descriptor.presentation.clone(),
                            window_title: descriptor.title.clone(),
                        });
                    } else {
                        self.log(format!("Existing source '{}' was not found", source_id));
                    }
                }
                SourceAction::PublishRecord(record) => {
                    let source_id = record.descriptor.source_id.clone();
                    let presentation = record.descriptor.presentation.clone();
                    let window_title = record.descriptor.title.clone();
                    self.result_source_store.insert_window_source(record);
                    self.emit(ExecutionEvent::OpenSourceWindow {
                        source_id,
                        presentation,
                        window_title,
                    });
                }
                SourceAction::PublishJson { presentation, data } => {
                    let source_id = format!("window_{}", uuid::Uuid::new_v4().simple());
                    match build_json_presentation_source(
                        source_id.clone(),
                        presentation,
                        &data,
                        Some(node_duration_ms),
                    ) {
                        Ok(record) => {
                            let presentation = record.descriptor.presentation.clone();
                            let window_title = record.descriptor.title.clone();
                            self.result_source_store.insert_window_source(record);
                            self.emit(ExecutionEvent::OpenSourceWindow {
                                source_id,
                                presentation,
                                window_title,
                            });
                        }
                        Err(err) => {
                            self.log(format!("Failed to store result source: {}", err));
                        }
                    }
                }
            }
        }

        result.map(|r| (r, node_duration_ms))
    }

    /// 节点开始执行时，为其所有已连线的 data input 发射 ConnectionActive。
    ///
    /// 使「某 input 用到某 output 的值」的连线都能在前端高亮（含同一 output
    /// 喂多个 input、Categorical 上游、已缓存值复用等场景）。
    fn emit_data_input_connections(&self, node_id: NodeId) {
        let connections: Vec<(String, String)> = {
            let graph = self.graph.lock().unwrap();
            graph
                .get_node_pins(node_id)
                .into_iter()
                .filter(|pin| pin.is_input() && pin.is_data())
                .filter_map(|pin| {
                    graph
                        .get_upstream_by_pin_id(pin.id)
                        .map(|upstream_pin_id| (upstream_pin_id.to_string(), pin.id.to_string()))
                })
                .collect()
        };

        for (from_pin_id, to_pin_id) in connections {
            self.emit(ExecutionEvent::ConnectionActive {
                from_pin_id,
                to_pin_id,
            });
        }
    }

    /// 递归执行所有上游的纯数据节点
    fn execute_upstream_data_nodes(&mut self, node_id: NodeId) -> Result<(), String> {
        let pins = {
            let graph = self.graph.lock().unwrap();
            graph.get_node_pins(node_id)
        };

        for pin in pins {
            if !pin.is_input() || !pin.is_data() {
                continue;
            }

            let upstream_info = {
                let graph = self.graph.lock().unwrap();
                graph
                    .get_upstream_by_pin_id(pin.id)
                    .and_then(|upstream_pin_id| {
                        graph
                            .get_node_id_by_pin_id(upstream_pin_id)
                            .map(|upstream_node_id| {
                                let upstream_definition =
                                    graph.get_node_definition_by_node_id(upstream_node_id);
                                (upstream_node_id, upstream_pin_id, upstream_definition)
                            })
                    })
            };

            if let Some((upstream_node_id, upstream_pin_id, upstream_definition)) = upstream_info {
                if upstream_definition.flow_processor.is_none()
                    && upstream_definition.data_evaluator.is_some()
                {
                    self.execute_upstream_data_nodes(upstream_node_id)?;

                    // 若上游输出 pin 已有有效值（同一执行内已被其他路径求值），则跳过，避免重复执行
                    // 仅检查 pins_runtime_state，不包含 resolve 的 default_value，否则 OneOf(Float64,DataSeries)
                    // 等类型的 default 会误判为已求值，导致 Add 等节点被跳过、Endog 收到 Float64
                    let already_has_value = {
                        let graph = self.graph.lock().unwrap();
                        graph.pin_has_executed_value(upstream_pin_id)
                    };
                    if already_has_value {
                        continue;
                    }

                    let upstream_node_id_str = upstream_node_id.to_string();
                    self.emit(ExecutionEvent::NodeStart {
                        node_id: upstream_node_id_str.clone(),
                    });
                    self.emit_data_input_connections(upstream_node_id);

                    let upstream_start = Instant::now();
                    let mut ctx = NodeExecutionContext::with_result_sources(
                        self.graph.clone(),
                        upstream_node_id,
                        self.result_source_store.clone(),
                        self.run_id.clone(),
                    );

                    let eval_result = if let Some(evaluator) = &upstream_definition.data_evaluator {
                        evaluator(&mut ctx)
                    } else {
                        Ok(())
                    };

                    self.logs.extend(ctx.logs);
                    for event in ctx.pin_result_events {
                        self.emit(event);
                    }
                    let upstream_duration_ms = upstream_start.elapsed().as_millis() as u64;

                    match eval_result {
                        Ok(()) => {
                            self.emit(ExecutionEvent::NodeComplete {
                                node_id: upstream_node_id_str,
                                duration_ms: upstream_duration_ms,
                            });
                        }
                        Err(err) => {
                            self.emit(ExecutionEvent::NodeError {
                                node_id: upstream_node_id_str,
                                error: err.clone(),
                                duration_ms: upstream_duration_ms,
                            });
                            return Err(err);
                        }
                    }
                }
            }
        }

        Ok(())
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
                // 本帧触发后立即完成，子帧应挂到「最近的 waiting 祖先」（frame.parent_frame），
                // 而非即将出栈的本帧；否则等待中的 Sequence/Branch 祖先会因看不到本分支
                // 的后续子树而提前恢复，导致 Then 分支交错执行。
                self.trigger_output_from_node_and_check(frame.node_id, role, frame.parent_frame)?;

                self.log(format!("Frame {:?} completed (after trigger)", frame.id));
                self.handle_frame_completion(frame)?;
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
                self.trigger_sequence(frame.clone(), roles)?;

                self.log(format!("Frame {:?} completed (after sequence)", frame.id));
                self.handle_frame_completion(frame)?;
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
    fn handle_frame_completion(&mut self, frame: ExecutionFrame) -> Result<(), String> {
        if let Some(parent_id) = frame.parent_frame {
            self.log(format!("Frame {:?} has parent {:?}", frame.id, parent_id));

            if !self.stack.has_active_children(parent_id) {
                self.log(format!("All children of {:?} completed", parent_id));

                let is_waiting = self
                    .stack
                    .get_frame(parent_id)
                    .map(|f| f.state == FrameState::WaitingForChild)
                    .unwrap_or(false);

                if is_waiting {
                    if let Some(mut parent_frame) = self.stack.remove(parent_id) {
                        self.log(format!("Resuming parent frame {:?}", parent_id));

                        if !parent_frame.remaining_continuations.is_empty() {
                            let current = parent_frame.remaining_continuations.remove(0);
                            let remaining =
                                std::mem::take(&mut parent_frame.remaining_continuations);

                            self.log(format!(
                                "Triggering {:?} with {} remaining",
                                current,
                                remaining.len()
                            ));
                            self.trigger_and_continue(parent_frame, current, remaining)?;
                        } else {
                            self.log(format!(
                                "Parent frame {:?} finished all continuations",
                                parent_id
                            ));
                            self.handle_frame_completion(parent_frame)?;
                        }
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

        let mut current_role = current;
        loop {
            let insert_index = self.stack.len();

            let has_downstream = self.trigger_output_from_node_and_check(
                node_id,
                current_role.clone(),
                Some(frame_id),
            )?;

            if has_downstream {
                frame.remaining_continuations = remaining;
                frame.state = FrameState::WaitingForChild;
                self.stack.insert_at(insert_index, frame);
                return Ok(());
            }

            self.log(format!(
                "No downstream for {:?}, processing next continuation immediately",
                current_role
            ));

            if remaining.is_empty() {
                self.log(format!(
                    "Frame {:?} completed (no more continuations)",
                    frame_id
                ));
                self.handle_frame_completion(frame)?;
                return Ok(());
            }

            current_role = remaining.remove(0);
        }
    }

    /// 触发输出并返回是否有下游连接，同时发送 ConnectionActive 事件
    fn trigger_output_from_node_and_check(
        &mut self,
        node_id: NodeId,
        role: ExecRole,
        parent_frame: Option<FrameId>,
    ) -> Result<bool, String> {
        let pin_role = PinRole::Exec(role);

        let (output_pin_id, downstream_pins, downstream_nodes) = {
            let graph = self.graph.lock().unwrap();
            let output_pin = graph
                .get_pin_instance_by_pin_role(node_id, &pin_role)
                .ok_or_else(|| {
                    format!("Output pin {:?} not found on node {:?}", pin_role, node_id)
                })?;

            let output_pin_id = output_pin.id;
            let downstream_pins = graph.get_downstream_by_pin_id(output_pin_id);

            if downstream_pins.is_empty() {
                return Ok(false);
            }

            let downstream_nodes: Vec<_> = downstream_pins
                .iter()
                .filter_map(|&pin_id| {
                    graph
                        .get_node_id_by_pin_id(pin_id)
                        .map(|node_id| (pin_id, node_id))
                })
                .collect();

            (output_pin_id, downstream_pins, downstream_nodes)
        };

        if downstream_pins.is_empty() {
            self.log(format!("No downstream connections for {:?}", pin_role));
            return Ok(false);
        }

        for (downstream_pin_id, downstream_node) in &downstream_nodes {
            self.log(format!(
                "Creating frame for downstream node {:?}",
                downstream_node
            ));

            self.emit(ExecutionEvent::ConnectionActive {
                from_pin_id: output_pin_id.to_string(),
                to_pin_id: downstream_pin_id.to_string(),
            });

            let frame_id =
                self.stack
                    .push_new(*downstream_node, Some(*downstream_pin_id), parent_frame);

            self.log(format!("Created frame {:?}", frame_id));
        }

        Ok(true)
    }

    /// 触发序列（逆序压栈）
    ///
    /// 本帧触发后立即完成，故各步子帧挂到 `frame.parent_frame`（最近的 waiting 祖先），
    /// 与 `TriggerOutput` 一致，避免祖先提前恢复。
    fn trigger_sequence(
        &mut self,
        frame: ExecutionFrame,
        roles: Vec<ExecRole>,
    ) -> Result<(), String> {
        for role in roles.into_iter().rev() {
            self.trigger_output_from_node(frame.node_id, role, frame.parent_frame)?;
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
        self.suspended_frames.insert(resume_token, frame_id);
        Ok(())
    }

    /// 恢复暂停的帧
    pub fn resume(&mut self, token: ResumeToken) -> Result<(), String> {
        let frame_id = self
            .suspended_frames
            .remove(&token)
            .ok_or_else(|| format!("Resume token {:?} not found", token))?;

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
            self.trigger_output(frame, body)?;
        } else {
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
