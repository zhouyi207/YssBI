//! 执行器（Executor）
//!
//! 执行器是唯一负责调度执行顺序的组件。它解释节点返回的 `ExecutionEffect`，
//! 通过「显式 join 作用域 + 子任务计数」协调 Sequence / Loop 等控制流的汇合：
//!
//! - spawn 下游帧时对其 `join_target` 的 `pending_children` **+1**（唯一 +1 处）
//! - 帧完成时对其 `join_target` 的 `pending_children` **-1**（唯一 -1 处）
//! - 当某 waiter 的 `pending_children` 归零且处于 `Waiting` → 恢复它
//!
//! 透明节点（`TriggerOutput`）把子帧挂到自己收到的 `join_target`（接力棒下传，
//! 保持同一作用域）；只有 waiter（Sequence / Loop）才新建作用域。

use self::wire_events::emit_exec_spawn;
use super::event_emitter::EventEmitter;
use super::execution_effect::ExecutionEffect;
use super::execution_event::ExecutionEvent;
use super::execution_frame::{ExecutionFrame, FrameId, FrameState, WaitKind};
use super::execution_stack::ExecutionStack;

mod data_inputs;
mod wire_events;
use crate::execution::{
    NodeExecutionContext, ResultSourceStore, SourceAction, build_json_presentation_source,
};
use crate::graph::GraphRuntime;
use crate::graph::node::NodeId;
use crate::graph::pin::{ExecRole, PinRole};
use crate::log_exec;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 执行器
///
/// 负责：
/// - 解释 ExecutionEffect
/// - 管理执行栈与 join 作用域
/// - 调度节点执行
/// - 通过 EventEmitter 流式发送执行事件给前端
pub struct Executor<E: EventEmitter> {
    /// 执行栈
    stack: ExecutionStack,

    /// Graph 引用
    graph: Arc<Mutex<GraphRuntime>>,

    /// 执行日志
    logs: Vec<String>,

    /// 事件发送器
    emitter: E,

    /// 结果 source 存储（窗口结果与运行时 pin 结果共用）
    result_source_store: ResultSourceStore,

    run_id: String,

    /// Cooperative cancellation flag used by the legacy executor.
    cancel: Option<Arc<AtomicBool>>,

    /// 首个节点失败后置位：不再 resume waiter / 清空待执行队列，避免 Sequence 继续 Then 3。
    halted: bool,

    /// 与 `halted` 配套的首个失败信息（供子程序向上传播）。
    failed_message: Option<String>,
}

impl<E: EventEmitter> Executor<E> {
    /// 创建新的执行器
    pub fn new(
        graph: Arc<Mutex<GraphRuntime>>,
        emitter: E,
        result_source_store: ResultSourceStore,
    ) -> Self {
        Self::with_cancel(graph, emitter, result_source_store, None)
    }

    pub fn with_cancel(
        graph: Arc<Mutex<GraphRuntime>>,
        emitter: E,
        result_source_store: ResultSourceStore,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self {
            stack: ExecutionStack::new(),
            graph,
            logs: Vec::new(),
            emitter,
            result_source_store,
            run_id: uuid::Uuid::new_v4().simple().to_string(),
            cancel,
            halted: false,
            failed_message: None,
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    }

    /// 发送执行事件
    fn emit(&self, event: ExecutionEvent) {
        self.emitter.emit(event);
    }

    pub(crate) fn absorb_pin_side_effects(&mut self, ctx: &mut NodeExecutionContext) {
        self.logs.append(&mut ctx.logs);
        for event in ctx.pin_result_events.drain(..) {
            self.emit(event);
        }
    }

    /// 开始执行（从指定节点开始）
    pub fn start(&mut self, entry_node: NodeId) -> Result<(), String> {
        self.log(format!("Starting execution from node {:?}", entry_node));

        // 创建根帧
        let frame_id = self.stack.push_ready(entry_node, None, None);
        self.log(format!("Created root frame {:?}", frame_id));

        // 执行主循环
        self.run()
    }

    /// 主执行循环（错误容错）
    fn run(&mut self) -> Result<(), String> {
        let graph_path = { self.graph.lock().unwrap().graph_path().as_str().to_string() };
        self.graph.lock().unwrap().reset_execution_state();
        self.result_source_store.clear_runtime_graph(&graph_path);
        self.emit(ExecutionEvent::ExecutionStart);

        match self.drain() {
            Ok(has_error) => {
                self.log("Execution completed".to_string());
                self.emit(ExecutionEvent::ExecutionComplete { has_error });
                Ok(())
            }
            Err(err) => {
                self.emit(ExecutionEvent::ExecutionComplete { has_error: true });
                Err(err)
            }
        }
    }

    /// 排空执行栈，返回是否发生过节点错误。不做 reset / clear / 生命周期事件，
    /// 供顶层 `run` 与子程序（函数调用）复用。
    fn drain(&mut self) -> Result<bool, String> {
        let mut has_error = false;

        while !self.stack.is_empty() {
            if self.is_cancelled() {
                self.log("Execution cancelled by user".to_string());
                return Err("Execution cancelled by user".to_string());
            }

            let mut frame = self.stack.pop_ready().ok_or("Stack is empty")?;
            self.log(format!("Executing frame {:?}", frame.id));
            frame.state = FrameState::Running;

            let node_id_str = frame.node_id.to_string();

            self.emit(ExecutionEvent::NodeStart {
                node_id: node_id_str.clone(),
            });

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
                    self.halted = true;
                    self.failed_message = Some(err.clone());
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
                    self.complete(frame)?;
                    self.stack.clear_ready();
                    break;
                }
            }
        }

        Ok(has_error)
    }

    /// 作为子程序从 `entry_node` 运行（函数调用用）。调用方负责在此之前
    /// `reset_execution_state` 并预置入参；这里不重置、不清 source、不发生命周期事件。
    pub fn run_subroutine(&mut self, entry_node: NodeId) -> Result<(), String> {
        self.stack.push_ready(entry_node, None, None);
        if self.drain()? {
            return Err(self
                .failed_message
                .take()
                .unwrap_or_else(|| "子图执行失败".to_string()));
        }
        Ok(())
    }

    /// 无 exec 入参的函数调用：仅按数据依赖求值 `node_id` 的上游。
    pub fn evaluate_data_target(&mut self, node_id: NodeId) -> Result<(), String> {
        self.satisfy_data_inputs(node_id)
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

        // 在执行节点之前，先满足全部 data input 依赖（取数 → 流动）
        self.satisfy_data_inputs(node_id).map_err(|e| (e, 0u64))?;

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

        // 收集日志与 pin 结果事件
        self.absorb_pin_side_effects(&mut ctx);

        let node_duration_ms = node_start.elapsed().as_millis() as u64;

        for action in ctx.source_actions {
            match action {
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

    /// 解释执行效果并更新栈
    fn interpret_effect(
        &mut self,
        effect: ExecutionEffect,
        frame: ExecutionFrame,
    ) -> Result<(), String> {
        match effect {
            ExecutionEffect::Done => {
                self.log(format!("Frame {:?} completed", frame.id));
                self.complete(frame)?;
            }

            // 透明节点：子帧挂到本帧收到的 join_target（接力棒下传，保持同一作用域），
            // 随后本帧完成。TriggerSequence 语义相同（多个输出同属父作用域）。
            ExecutionEffect::TriggerOutput(role) => {
                self.log(format!("Triggering output {:?}", role));
                self.spawn(frame.node_id, role, frame.join_target)?;
                self.complete(frame)?;
            }

            ExecutionEffect::TriggerSequence(roles) => {
                self.log(format!("Triggering sequence of {} outputs", roles.len()));
                for role in roles.into_iter().rev() {
                    self.spawn(frame.node_id, role, frame.join_target)?;
                }
                self.complete(frame)?;
            }

            // Sequence：新建作用域，等待 current 分支整棵子树排空后再触发下一个 Then。
            ExecutionEffect::TriggerAndContinue { current, remaining } => {
                self.log(format!(
                    "Sequence {:?}: firing {:?}, {} remaining",
                    frame.id,
                    current,
                    remaining.len()
                ));
                let mut roles = Vec::with_capacity(1 + remaining.len());
                roles.push(current);
                roles.extend(remaining);
                self.begin_wait(frame, WaitKind::Continuation { remaining: roles })?;
            }

            // Loop：新建作用域，等待 body 整棵子树排空后重跑循环节点。
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

    /// 帧完成：对其 join_target 计数 -1（唯一 -1 处）；归零且处于 Waiting 时恢复。
    fn complete(&mut self, frame: ExecutionFrame) -> Result<(), String> {
        let Some(owner_id) = frame.join_target else {
            return Ok(());
        };

        let should_resume = {
            let Some(owner) = self.stack.get_mut(owner_id) else {
                return Ok(());
            };
            owner.pending_children = owner.pending_children.saturating_sub(1);
            let pending = owner.pending_children;
            let resume = pending == 0 && owner.state == FrameState::Waiting;
            self.log(format!(
                "Frame {:?} done -> owner {:?} pending={}",
                frame.id, owner_id, pending
            ));
            resume
        };

        if should_resume && !self.halted {
            self.resume(owner_id)?;
        }
        Ok(())
    }

    /// 让一个 waiter 帧进入等待：park 到 frames，并触发它要等待的分支。
    ///
    /// 若该分支没有任何下游（pending 仍为 0），立即恢复（避免死等）。
    fn begin_wait(&mut self, mut frame: ExecutionFrame, wait: WaitKind) -> Result<(), String> {
        let owner_id = frame.id;
        frame.state = FrameState::Waiting;
        frame.pending_children = 0;
        frame.wait = Some(wait);
        self.stack.park(frame);

        self.dispatch_wait(owner_id)
    }

    /// 触发 Sequence 当前要等待的 Then 分支；无下游时立即推进到下一个，
    /// 全部耗尽则收尾 waiter。仅用于 `WaitKind::Continuation`。
    fn dispatch_wait(&mut self, owner_id: FrameId) -> Result<(), String> {
        loop {
            let (node_id, kind) = {
                let owner = self
                    .stack
                    .get(owner_id)
                    .ok_or_else(|| format!("Waiter {:?} not found", owner_id))?;
                (owner.node_id, owner.wait.clone())
            };

            let Some(WaitKind::Continuation { mut remaining }) = kind else {
                return Ok(());
            };

            if remaining.is_empty() {
                // 所有 Then 已耗尽 -> Sequence 完成
                return self.finish_waiter(owner_id);
            }
            let current = remaining.remove(0);
            if let Some(owner) = self.stack.get_mut(owner_id) {
                owner.wait = Some(WaitKind::Continuation { remaining });
            }
            let spawned = self.spawn(node_id, current.clone(), Some(owner_id))?;
            if spawned == 0 {
                self.log(format!(
                    "Sequence {:?}: {:?} has no downstream, advancing",
                    owner_id, current
                ));
                continue;
            }
            return Ok(());
        }
    }

    /// waiter 全部子任务完成后的恢复入口。
    fn resume(&mut self, owner_id: FrameId) -> Result<(), String> {
        let kind = self.stack.get(owner_id).and_then(|f| f.wait.clone());
        match kind {
            Some(WaitKind::Continuation { .. }) => {
                self.log(format!("Resuming sequence {:?}", owner_id));
                self.dispatch_wait(owner_id)
            }
            Some(WaitKind::LoopReentry) => {
                self.log(format!("Loop body drained, re-evaluating {:?}", owner_id));
                self.reenter_loop(owner_id)
            }
            None => Ok(()),
        }
    }

    /// 循环重入：把循环节点重新入队执行（计数器已在节点内推进）。
    fn reenter_loop(&mut self, owner_id: FrameId) -> Result<(), String> {
        let Some(owner) = self.stack.remove(owner_id) else {
            return Ok(());
        };
        self.stack
            .push_ready(owner.node_id, owner.triggered_by, owner.join_target);
        Ok(())
    }

    /// waiter 彻底完成：从 frames 移除，并像普通帧一样对其 join_target 收尾。
    fn finish_waiter(&mut self, owner_id: FrameId) -> Result<(), String> {
        let Some(mut owner) = self.stack.remove(owner_id) else {
            return Ok(());
        };
        owner.state = FrameState::Completed;
        owner.wait = None;
        self.log(format!("Waiter {:?} finished", owner_id));
        self.complete(owner)
    }

    /// spawn 下游帧：为 role 的每个下游创建 Ready 帧，并对 join_target 计数 +1。
    ///
    /// 返回创建的下游帧数量（0 表示该输出无连接）。这是 `pending_children` 的唯一 +1 处。
    /// exec 连线视觉事件由 `wire_events::emit_exec_spawn` 统一发射。
    fn spawn(
        &mut self,
        node_id: NodeId,
        role: ExecRole,
        join_target: Option<FrameId>,
    ) -> Result<usize, String> {
        let pin_role = PinRole::Exec(role);

        let (output_pin_id, downstream_nodes) = {
            let graph = self.graph.lock().unwrap();
            let output_pin = graph
                .get_pin_instance_by_pin_role(node_id, &pin_role)
                .ok_or_else(|| {
                    format!("Output pin {:?} not found on node {:?}", pin_role, node_id)
                })?;

            let output_pin_id = output_pin.id;
            let downstream_nodes: Vec<_> = graph
                .get_downstream_by_pin_id(output_pin_id)
                .into_iter()
                .filter_map(|pin_id| {
                    graph
                        .get_node_id_by_pin_id(pin_id)
                        .map(|node_id| (pin_id, node_id))
                })
                .collect();

            (output_pin_id, downstream_nodes)
        };

        if downstream_nodes.is_empty() {
            self.log(format!("No downstream connections for {:?}", pin_role));
            return Ok(0);
        }

        for (downstream_pin_id, downstream_node) in &downstream_nodes {
            emit_exec_spawn(&self.emitter, output_pin_id, *downstream_pin_id);

            let frame_id =
                self.stack
                    .push_ready(*downstream_node, Some(*downstream_pin_id), join_target);

            if let Some(owner_id) = join_target {
                if let Some(owner) = self.stack.get_mut(owner_id) {
                    owner.pending_children += 1;
                }
            }

            self.log(format!(
                "Spawned frame {:?} for node {:?} (owner {:?})",
                frame_id, downstream_node, join_target
            ));
        }

        Ok(downstream_nodes.len())
    }

    /// 处理循环
    ///
    /// - 继续：新建作用域 (LoopReentry)，park 并触发 body；body 无下游则直接走完成路径。
    /// - 停止：触发 completed 到父作用域，本帧完成。
    fn handle_loop(
        &mut self,
        frame: ExecutionFrame,
        body: ExecRole,
        completed: ExecRole,
        should_continue: bool,
    ) -> Result<(), String> {
        if !should_continue {
            self.log("Loop finished, triggering completed".to_string());
            self.spawn(frame.node_id, completed, frame.join_target)?;
            return self.complete(frame);
        }

        let node_id = frame.node_id;
        let owner_id = frame.id;

        // 进入 body 作用域：先 park 为 waiter，再 spawn body 到该作用域。
        let mut waiter = frame;
        waiter.state = FrameState::Waiting;
        waiter.pending_children = 0;
        waiter.wait = Some(WaitKind::LoopReentry);
        let join_target = waiter.join_target;
        self.stack.park(waiter);

        self.log(format!("Loop {:?} triggering body {:?}", owner_id, body));
        let spawned = self.spawn(node_id, body, Some(owner_id))?;

        if spawned == 0 {
            // body 无下游：不进入等待，直接触发 completed 到父作用域并完成。
            self.log("Loop body has no downstream, triggering completed".to_string());
            let waiter = self
                .stack
                .remove(owner_id)
                .ok_or_else(|| format!("Loop waiter {:?} lost", owner_id))?;
            self.spawn(node_id, completed, join_target)?;
            return self.complete(waiter);
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
