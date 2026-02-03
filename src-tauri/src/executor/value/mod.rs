//! 值系统模块
//!
//! 定义数据类型、值表示和类型推断系统

pub mod data_type;
pub mod data_value;
pub mod pin_type;
pub mod type_inference;

pub use data_type::*;
pub use data_value::*;
pub use pin_type::*;
pub use type_inference::*;
