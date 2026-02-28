use polars::prelude::*;
use std::sync::Arc;
use yss_sci::database::EditHistory;

pub enum DatabaseState {
    Lazy { lazy_frame: LazyFrame },

    Loaded {
        dataframe: Arc<DataFrame>,
        original: Arc<DataFrame>,
        history: EditHistory,
    },

    Failed { error: String },
}
