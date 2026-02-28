//! 测试公共辅助函数

use std::sync::{Arc, Mutex};
use yssbi_lib::execution::{Executor, NoopEmitter, WindowDataStore};
use yssbi_lib::graph::core::GraphRuntime;

/// 创建用于测试的执行器（无需 Tauri Channel）
pub fn executor_for_test(graph: Arc<Mutex<GraphRuntime>>) -> Executor<NoopEmitter> {
    Executor::new(graph, NoopEmitter, WindowDataStore::new())
}
