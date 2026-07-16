//! data input 依赖求值 — 取数 / 流动的唯一业务入口。
//!
//! 每条已连线的 data input 边都走 `satisfy_data_input_edge`：
//! 1. `emit_data_pull`（取数）
//! 2. 递归满足上游节点的 data 依赖
//! 3. 必要时求值可拉取的纯数据节点
//! 4. `emit_data_flow`（流动，仅当 upstream output pin 已有执行期值）

use super::super::event_emitter::EventEmitter;
use super::super::execution_event::ExecutionEvent;
use super::Executor;
use super::wire_events::{emit_data_flow, emit_data_pull};
use crate::execution::NodeExecutionContext;
use crate::graph::PinId;
use crate::graph::node::NodeId;
use crate::graph::pin::PinInstance;
use std::time::Instant;

struct WiredDataInput {
    input_pin: PinId,
    upstream_pin: PinId,
    upstream_node: NodeId,
}

impl<E: EventEmitter> Executor<E> {
    /// 在消费者运行前，满足其全部已连线 data input（取数 → 求值 → 流动）。
    pub(crate) fn satisfy_data_inputs(&mut self, consumer_id: NodeId) -> Result<(), String> {
        let edges = self.collect_wired_data_inputs(consumer_id);
        for edge in edges {
            self.satisfy_data_input_edge(edge)?;
        }
        Ok(())
    }

    fn collect_wired_data_inputs(&self, consumer_id: NodeId) -> Vec<WiredDataInput> {
        let graph = self.graph.lock().unwrap();
        graph
            .get_node_pins(consumer_id)
            .into_iter()
            .filter(|pin: &PinInstance| pin.is_input() && pin.is_data())
            .filter_map(|input_pin| {
                let upstream_pin = graph.get_upstream_by_pin_id(input_pin.id)?;
                let upstream_node = graph.get_node_id_by_pin_id(upstream_pin)?;
                Some(WiredDataInput {
                    input_pin: input_pin.id,
                    upstream_pin,
                    upstream_node,
                })
            })
            .collect()
    }

    fn satisfy_data_input_edge(&mut self, edge: WiredDataInput) -> Result<(), String> {
        emit_data_pull(&self.emitter, edge.upstream_pin, edge.input_pin);

        if self.is_pullable_data_node(edge.upstream_node) {
            self.satisfy_data_inputs(edge.upstream_node)?;

            if self.emit_data_flow_if_ready(&edge) {
                return Ok(());
            }

            self.evaluate_pullable_node(edge.upstream_node)?;
            emit_data_flow(&self.emitter, edge.upstream_pin, edge.input_pin);
            return Ok(());
        }

        self.emit_data_flow_if_ready(&edge);
        Ok(())
    }

    fn is_pullable_data_node(&self, node_id: NodeId) -> bool {
        let graph = self.graph.lock().unwrap();
        let definition = graph.get_node_definition_by_node_id(node_id);
        definition.data_evaluator.is_some() && !graph.node_has_exec_pins(node_id)
    }

    fn emit_data_flow_if_ready(&self, edge: &WiredDataInput) -> bool {
        if !self.pin_has_executed_value(edge.upstream_pin) {
            return false;
        }
        emit_data_flow(&self.emitter, edge.upstream_pin, edge.input_pin);
        true
    }

    fn pin_has_executed_value(&self, pin_id: PinId) -> bool {
        self.graph.lock().unwrap().pin_has_executed_value(pin_id)
    }

    /// 求值可拉取纯数据节点（其 data input 须已由 `satisfy_data_inputs` 满足）。
    fn evaluate_pullable_node(&mut self, node_id: NodeId) -> Result<(), String> {
        let definition = {
            let graph = self.graph.lock().unwrap();
            graph.get_node_definition_by_node_id(node_id)
        };

        let node_id_str = node_id.to_string();
        self.emit(ExecutionEvent::NodeStart {
            node_id: node_id_str.clone(),
        });

        let node_start = Instant::now();
        let mut ctx = NodeExecutionContext::with_result_sources(
            self.graph.clone(),
            node_id,
            self.result_source_store.clone(),
            self.run_id.clone(),
        );

        let eval_result = if let Some(evaluator) = &definition.data_evaluator {
            evaluator(&mut ctx)
        } else {
            Ok(())
        };

        self.absorb_pin_side_effects(&mut ctx);
        let duration_ms = node_start.elapsed().as_millis() as u64;

        match eval_result {
            Ok(()) => {
                self.emit(ExecutionEvent::NodeComplete {
                    node_id: node_id_str,
                    duration_ms,
                });
                Ok(())
            }
            Err(err) => {
                self.emit(ExecutionEvent::NodeError {
                    node_id: node_id_str,
                    error: err.clone(),
                    duration_ms,
                });
                Err(err)
            }
        }
    }
}
