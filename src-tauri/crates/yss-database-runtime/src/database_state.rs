//! Fixed dataset handles and session-scoped undo/redo history.
use std::sync::Arc;
use yss_database_edit::EditHistory;
use yss_datafusion::DataFusionRuntime;
use yss_dataset_store::DatasetSnapshot;

#[derive(Clone)]
pub struct DatasetEdit {
    pub(crate) before: Arc<DatasetSnapshot>,
    pub(crate) after: Arc<DatasetSnapshot>,
}

#[derive(Clone)]
pub enum DatabaseState {
    Dataset {
        snapshot: Arc<DatasetSnapshot>,
        engine: Arc<DataFusionRuntime>,
        history: EditHistory<DatasetEdit>,
    },
    Failed {
        error: String,
    },
}
