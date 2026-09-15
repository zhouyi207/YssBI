//! Fixed dataset handles and session-scoped undo/redo history.
use crate::edit_history::EditHistory;
use std::sync::Arc;
use yss_datafusion::DataFusionRuntime;
use yss_dataset_store::DatasetSnapshot;

#[derive(Clone)]
pub(crate) struct DatasetEdit {
    pub(crate) before: Arc<DatasetSnapshot>,
    pub(crate) after: Arc<DatasetSnapshot>,
}

#[derive(Clone)]
pub(crate) enum DatabaseState {
    Dataset {
        snapshot: Arc<DatasetSnapshot>,
        engine: Arc<DataFusionRuntime>,
        history: EditHistory<DatasetEdit>,
    },
    Failed,
}
