pub mod database_access;
pub mod database_decl;
pub mod database_engine;
pub mod database_engine_sql;
pub mod database_error;
pub mod database_instance;
pub mod database_state;
pub mod database_view;

pub use database_access::*;
pub use database_decl::*;
pub use database_engine::*;
pub use database_engine_sql::*;
pub use database_error::*;
pub use database_instance::*;
pub use database_state::*;
pub use database_view::*;

use polars::prelude::*;

pub struct PreviewRow {
    pub cells: Vec<String>,
}

pub fn dataframe_to_preview_rows(df: &DataFrame) -> Vec<PreviewRow> {
    let height = df.height();
    let columns = df.get_columns();

    (0..height)
        .map(|row_idx| PreviewRow {
            cells: columns
                .iter()
                .map(|col| col.get(row_idx).map(|v| v.to_string()).unwrap_or_default())
                .collect(),
        })
        .collect()
}
