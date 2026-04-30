//! Frontend 工具模块
//!
//! 提供后端 → 前端的错误转换类型。
//! FrontendError 将内部 ProjectError 等转为前端友好的 { code, message } 格式。

pub mod frontend_error;

pub use frontend_error::*;
