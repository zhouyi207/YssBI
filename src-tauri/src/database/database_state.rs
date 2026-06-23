use polars::prelude::*;
use std::sync::Arc;
use yss_sci::api::database::EditHistory;

use super::DuckDbColumnMeta;

/// 数据库实例的生命周期状态。
///
/// - `Pending`：仅持有声明（`DatabaseInstance::decl`），尚未触发任何 IO；
///   多用于 SQL / Excel 这类 Polars 不支持真正惰性读取的引擎，避免在
///   `set_data` 阶段同步阻塞用户。第一次访问时由 `DatabaseInstance` 调用
///   `decl.engine.build_lazy()` 物化为 `Loaded`（或失败时进入 `Failed`）。
/// - `Lazy`：Polars 原生惰性计划（Parquet）。schema 与 row_count 按需读取。
/// - `DuckDb`：项目内 DuckDB 列存，元数据已缓存，IO 走 `duckdb_reader`。
/// - `Loaded`：完整的 DataFrame 已经在内存中，可被编辑。
/// - `Failed`：上一次 IO 失败，错误信息保存在内。
pub enum DatabaseState {
    Pending,

    Lazy {
        lazy_frame: LazyFrame,
    },

    DuckDb {
        /// 运行时绝对路径（decl 中仍保存相对项目根的路径）
        duckdb_path: String,
        table: String,
        row_count: usize,
        columns: Vec<DuckDbColumnMeta>,
    },

    Loaded {
        dataframe: Arc<DataFrame>,
        original: Arc<DataFrame>,
        history: EditHistory,
    },

    Failed {
        error: String,
    },
}
