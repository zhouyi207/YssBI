use polars::prelude::*;
use std::sync::Arc;
use yss_sci::api::database::EditHistory;

use super::DuckDbColumnMeta;

/// 数据库实例的生命周期状态。
///
/// - `DuckDb`：项目内 DuckDB 列存，元数据已缓存，IO 走 `duckdb_reader`。
/// - `Loaded`：内存中的 DataFrame（编辑会话），可通过 `save_changes` 写回 DuckDB。
/// - `Failed`：上一次 IO 失败，错误信息保存在内。
pub enum DatabaseState {
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
