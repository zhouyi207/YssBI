//! Value 模块 - 基于 Polars/Arrow 的类型系统
//!
//! 提供统一的数据类型定义和转换功能，用于 BI 系统的数据处理

pub mod types;
pub mod conversions;

pub use types::{Value, ValueType};
pub use conversions::{from_json, to_json, from_polars, to_polars};
