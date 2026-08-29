//! 时间序列模块
//!
//! 基于 Polars Series 的计量/时间序列工具：
//! - 不依赖 DataFrame，只依赖 ndarray + polars
//! - lag 严格按时间对齐（Stata L. 语义）
//! - 支持数字时间 + 日期时间
//! - O(n) 复杂度（rolling 为 O(n×w)）

pub(crate) mod distributions;

pub mod acf_pacf;
pub mod align;
pub mod diff;
pub mod lag;
pub mod pct_change;
pub mod rolling;
pub mod serial_correlation;
pub mod types;
pub mod unit_root;
pub mod var;
pub mod vec;
pub mod vec_vecrank_cv;
