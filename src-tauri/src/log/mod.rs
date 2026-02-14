pub mod log_level;
pub mod log_manager;
pub mod log_message;
pub mod log_type;
pub mod macros;

#[cfg(test)]
mod examples;

pub use log_level::*;
pub use log_manager::*;
pub use log_message::*;
pub use log_type::*;

// 重新导出宏模块，使其可以通过 crate::log::log_app::info! 调用
pub use macros::{log_app, log_exec, log_sys};
