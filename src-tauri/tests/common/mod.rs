//! 测试公共辅助函数
#![allow(dead_code)] // 各集成测试 crate 各自取用部分 helper，未用到的不算死代码

use std::sync::{Arc, Mutex};
use yssbi_lib::execution::{
    EventEmitter, ExecutionEvent, Executor, NoopEmitter, ResultSourceStore,
};
use yssbi_lib::graph::core::GraphRuntime;

/// 创建用于测试的执行器（无需 Tauri Channel）
pub fn executor_for_test(graph: Arc<Mutex<GraphRuntime>>) -> Executor<NoopEmitter> {
    Executor::new(graph, NoopEmitter, ResultSourceStore::new())
}

/// 记录型事件发送器：按 `NodeStart` 顺序收集 node_id，用于断言执行顺序
#[derive(Clone, Default)]
pub struct RecordingEmitter {
    order: Arc<Mutex<Vec<String>>>,
}

impl RecordingEmitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 已执行节点的顺序快照（按 NodeStart 触发先后）
    pub fn order(&self) -> Vec<String> {
        self.order.lock().unwrap().clone()
    }
}

impl EventEmitter for RecordingEmitter {
    fn emit(&self, event: ExecutionEvent) {
        if let ExecutionEvent::NodeStart { node_id } = event {
            self.order.lock().unwrap().push(node_id);
        }
    }
}

/// 创建带顺序记录的执行器，返回 (executor, emitter) —— emitter 持有共享记录句柄
pub fn recording_executor_for_test(
    graph: Arc<Mutex<GraphRuntime>>,
) -> (Executor<RecordingEmitter>, RecordingEmitter) {
    let emitter = RecordingEmitter::new();
    let executor = Executor::new(graph, emitter.clone(), ResultSourceStore::new());
    (executor, emitter)
}
