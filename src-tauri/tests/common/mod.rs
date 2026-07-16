//! 测试公共辅助函数
#![allow(dead_code)] // 各集成测试 crate 各自取用部分 helper，未用到的不算死代码

use std::sync::{Arc, Mutex};
use yssbi_lib::execution::{
    EventEmitter, ExecutionEvent, Executor, NoopEmitter, ResultSourceStore,
};
use yssbi_lib::graph::FunctionSignaturePin;
use yssbi_lib::graph::core::GraphRuntime;
use yssbi_lib::graph::value::DataType;

/// 测试夹具：从简短类型名构建结构化 `FunctionSignaturePin`（`exec` / `int` / `float` 等）。
pub fn function_signature_pin(id: &str, name: &str, pin_type: &str) -> FunctionSignaturePin {
    if pin_type.eq_ignore_ascii_case("exec") {
        return FunctionSignaturePin::exec(id, name);
    }
    let dt = match pin_type {
        "int" => DataType::Int64,
        "float" => DataType::Float64,
        "bool" => DataType::Boolean,
        "string" => DataType::String,
        "object" => DataType::Object,
        other => other.parse().unwrap_or(DataType::Any),
    };
    FunctionSignaturePin::data(id, name, dt)
}

/// 创建用于测试的执行器（无需 Tauri Channel）
pub fn executor_for_test(graph: Arc<Mutex<GraphRuntime>>) -> Executor<NoopEmitter> {
    Executor::new(graph, NoopEmitter, ResultSourceStore::new())
}

/// 集成测试用事件收集器：可同时记录 `NodeStart` 顺序与 `OpenSourceWindow` 的 source_id。
#[derive(Clone, Default)]
pub struct CapturingEmitter {
    node_starts: Arc<Mutex<Vec<String>>>,
    window_source_ids: Arc<Mutex<Vec<String>>>,
}

impl CapturingEmitter {
    pub fn node_starts(&self) -> Vec<String> {
        self.node_starts.lock().unwrap().clone()
    }

    /// 与历史 `RecordingEmitter::order()` 兼容的别名。
    pub fn order(&self) -> Vec<String> {
        self.node_starts()
    }

    pub fn window_source_ids(&self) -> Vec<String> {
        self.window_source_ids.lock().unwrap().clone()
    }
}

impl EventEmitter for CapturingEmitter {
    fn emit(&self, event: ExecutionEvent) {
        match event {
            ExecutionEvent::NodeStart { node_id } => {
                self.node_starts.lock().unwrap().push(node_id);
            }
            ExecutionEvent::OpenSourceWindow { source_id, .. } => {
                self.window_source_ids.lock().unwrap().push(source_id);
            }
            _ => {}
        }
    }
}

/// 创建带事件收集与可共享 ResultSourceStore 的执行器。
pub fn capturing_executor_for_test(
    graph: Arc<Mutex<GraphRuntime>>,
    store: ResultSourceStore,
) -> (Executor<CapturingEmitter>, CapturingEmitter) {
    let emitter = CapturingEmitter::default();
    let executor = Executor::new(graph, emitter.clone(), store);
    (executor, emitter)
}

/// 仅断言节点执行顺序时使用（内部 store 不可外读）。
pub fn recording_executor_for_test(
    graph: Arc<Mutex<GraphRuntime>>,
) -> (Executor<CapturingEmitter>, CapturingEmitter) {
    capturing_executor_for_test(graph, ResultSourceStore::new())
}
