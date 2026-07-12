//! 执行系统（Execution System）
//!
//! 基于 continuation 的执行架构，包含：
//! - ExecutionEffect - 节点返回的执行效果
//! - ExecutionFrame - 执行帧
//! - ExecutionStack - 执行栈
//! - Executor - 执行器

pub mod event_emitter;
pub mod execution_effect;
pub mod execution_event;
pub mod execution_frame;
pub mod execution_stack;
pub mod executor;

pub use event_emitter::{ChannelEventEmitter, EventEmitter, NoopEmitter};
pub use execution_effect::ExecutionEffect;
pub use execution_event::ExecutionEvent;
pub use execution_frame::{ExecutionFrame, FrameId, FrameState, WaitKind};
pub use execution_stack::ExecutionStack;
pub use executor::Executor;
