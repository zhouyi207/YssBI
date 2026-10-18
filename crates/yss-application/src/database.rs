use std::sync::Arc;

use yss_database_contract::DatabaseId;
use yss_project_identity::ProjectInstanceId;

use crate::events::CommittedResourceMutation;
use crate::session::{ApplicationSession, ApplicationState};

mod edit;
mod error;
mod export;
mod import;
mod mutation;
mod query;
pub mod samples;
#[cfg(test)]
mod tests;

pub use edit::DatabaseMutation;
pub use error::{
    DatabaseApplicationInternalError, DatabaseApplicationOperation, DatabaseMutationFailure,
    DatabaseOperationError, DatabaseUseCaseError,
};
pub use import::{LoadDatabaseResult, list_excel_sheets, list_sql_tables, list_sqlite_tables};
pub use query::{DatabaseMetaResult, DatabaseRowsResult};

#[derive(Debug)]
pub struct DatabaseMutationResult<T> {
    pub data: T,
    pub mutation: CommittedResourceMutation,
}

#[derive(Debug)]
pub struct DatabaseEditResult {
    pub edit_state: yss_database_contract::EditState,
    pub inserted_row_ids: Vec<i64>,
}

impl ApplicationState {
    fn capture_database_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, DatabaseUseCaseError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(DatabaseUseCaseError::Database(
                DatabaseOperationError::StaleProject {
                    project_instance_id: project_instance_id.clone(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_database_session(
        &self,
        captured: &Arc<ApplicationSession>,
    ) -> Result<(), DatabaseUseCaseError> {
        self.rebuild_application_session(captured)
            .map_err(DatabaseUseCaseError::SessionRefresh)
    }
}

fn database_id(id: &str) -> DatabaseId {
    DatabaseId::from_existing(id.to_owned().into_boxed_str())
}
