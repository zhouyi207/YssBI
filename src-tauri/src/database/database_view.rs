use super::PreviewRow;
use polars::prelude::*;

pub enum DatabaseView {
    /// 只允许小规模读取
    Preview {
        rows: Vec<PreviewRow>,
        row_count: usize,
        column_count: usize,
    },

    /// 执行期使用（不直接暴露 &DataFrame 给外部系统）
    Execution { dataframe:  DataFrame },
}
