use super::ProjectState;
use crate::database::*;
use polars::prelude::*;

/// let preview = project_state
///    .access_database("sales", DatabaseAccess::Preview)?;
impl ProjectState {
    pub fn access_database(&self, id: &str, access: DatabaseAccess) -> PolarsResult<DatabaseView> {
        let mut store = self.project_store.write().unwrap();
        let db = store
            .databases
            .get_mut(id)
            .ok_or_else(|| PolarsError::NoData("nodata".into()))?;

        db.access(access)
    }
}
