use crate::edit_history::EditHistory;
use crate::{DatabaseInstance, DatabaseState};
use std::sync::{Arc, OnceLock};
use yss_database_contract::DatabaseDecl;
use yss_database_engine::DataFusionRuntime;
use yss_database_store::{DatasetStore, DatasetStoreError};
use yss_relational_contract::RelationError;

pub fn dataset_query_engine() -> Result<Arc<DataFusionRuntime>, DatasetStoreError> {
    static ENGINE: OnceLock<Result<Arc<DataFusionRuntime>, RelationError>> = OnceLock::new();
    ENGINE
        .get_or_init(|| DataFusionRuntime::new(512 * 1024 * 1024, 8192))
        .clone()
        .map_err(Into::into)
}

pub fn bind_dataset_instance(decl: &DatabaseDecl, store: &Arc<DatasetStore>) -> DatabaseInstance {
    let state = match store
        .snapshot(&decl.id)
        .and_then(|snapshot| Ok((snapshot, dataset_query_engine()?)))
    {
        Ok((snapshot, engine)) => DatabaseState::Dataset {
            snapshot,
            engine,
            history: EditHistory::new(),
        },
        Err(_) => DatabaseState::Failed,
    };
    DatabaseInstance {
        decl: decl.clone(),
        state,
    }
}
