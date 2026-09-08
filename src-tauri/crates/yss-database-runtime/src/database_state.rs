//! Cross-engine physical database state held by a runtime session.

use yss_database_edit::EditHistory;
use yss_duckdb::DuckDbColumnMeta;

/// 数据库实例的生命周期状态。
///
/// - `DuckDb`：项目内 DuckDB 列存，元数据已缓存；编辑走 SQL + `history`，不整表 Loaded。
/// - `Failed`：上一次 IO 失败，错误信息保存在内。
#[derive(Clone)]
pub enum DatabaseState {
    DuckDb {
        /// 运行时绝对路径（decl 中仍保存相对项目根的路径）
        duckdb_path: String,
        table: String,
        row_count: usize,
        columns: Vec<DuckDbColumnMeta>,
        history: EditHistory,
    },

    Failed {
        error: String,
    },
}
