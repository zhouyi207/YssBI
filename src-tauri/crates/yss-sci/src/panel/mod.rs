//! Panel data operations: align, diff
//!
//! Align: 按 (entity, time) 补齐到规则时间网格，缺失为 NaN。
//! Diff: 在 align 后的数据上，按 entity 分组对相邻时间做一阶差分。

pub mod align;

pub use align::{AlignedPanel, align_dataframe, align_panel, diff_dataframe, panel_diff};
