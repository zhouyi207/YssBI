use polars::prelude::*;
use std::sync::Arc;
use yss_sci::api::database::EditHistory;

use super::DuckDbColumnMeta;

/// 数据库实例的生命周期状态。
///
/// - `DuckDb`：项目内 DuckDB 列存，元数据已缓存；编辑走 SQL + `history`，不整表 Loaded。
/// - `Loaded`：小表内存编辑会话（`row_count <= MAX_IN_MEMORY_EDIT_ROWS`），可 `save_changes` 全量写回。
/// - `Failed`：上一次 IO 失败，错误信息保存在内。
pub enum DatabaseState {
    DuckDb {
        /// 运行时绝对路径（decl 中仍保存相对项目根的路径）
        duckdb_path: String,
        table: String,
        row_count: usize,
        columns: Vec<DuckDbColumnMeta>,
        history: EditHistory,
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
