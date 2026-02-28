//! 执行事件发送器抽象
//!
//! 用于解耦 Executor 与 Tauri Channel，使测试无需 Tauri 上下文即可运行

use super::execution_event::ExecutionEvent;
use tauri::ipc::Channel;

/// 执行事件发送器
///
/// 生产环境由 Tauri Channel 实现，测试环境使用 NoopEmitter 丢弃事件
pub trait EventEmitter: Send {
    fn emit(&self, event: ExecutionEvent);
}

/// 空实现，用于测试
#[derive(Clone, Default)]
pub struct NoopEmitter;

impl EventEmitter for NoopEmitter {
    fn emit(&self, _event: ExecutionEvent) {}
}

/// 包装 Tauri Channel，用于生产环境
pub struct ChannelEventEmitter(pub Channel<ExecutionEvent>);

impl EventEmitter for ChannelEventEmitter {
    fn emit(&self, event: ExecutionEvent) {
        if let Err(e) = self.0.send(event) {
            eprintln!("[Executor] Channel send failed: {}", e);
        }
    }
}
