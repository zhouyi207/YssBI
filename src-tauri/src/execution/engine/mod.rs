//! 执行系统（Execution System）
//!
//! 基于 continuation 的执行架构，包含：
//! - ExecutionEffect - 节点返回的执行效果
//! - ExecutionFrame - 执行帧
//! - ExecutionStack - 执行栈
//! - Executor - 执行器

pub mod execution_effect;
pub mod execution_frame;
pub mod execution_stack;
pub mod executor;

pub use execution_effect::{ExecutionEffect, ResumeToken};
pub use execution_frame::{ExecutionFrame, FrameId, FrameState};
pub use execution_stack::ExecutionStack;
pub use executor::Executor;
