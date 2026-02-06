use polars::prelude::*;
use std::sync::Arc;

pub enum DatabaseState {
    /// 尚未真正加载，仅有 Lazy 执行计划
    Lazy { lazy_frame: LazyFrame },

    /// 已经 collect 进内存（节点执行结果 / 显式加载）
    Loaded { dataframe: Arc<DataFrame> },

    /// 加载失败（避免反复 IO）
    Failed { error: String },
}
