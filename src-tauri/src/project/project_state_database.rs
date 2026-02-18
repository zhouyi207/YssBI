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

    /// 添加数据库到 project_store 和 project_data
    pub fn add_database(&self, instance: DatabaseInstance) {
        let decl = instance.decl.clone();
        let id = decl.id.clone();
        {
            let mut store = self.project_store.write().unwrap();
            store.databases.insert(id.clone(), instance);
        }
        {
            let mut data = self.project_data.write().unwrap();
            data.databases.insert(id, decl);
        }
    }

    /// 从 project_store 和 project_data 中移除数据库
    pub fn delete_database(&self, id: &str) {
        {
            let mut store = self.project_store.write().unwrap();
            store.databases.remove(id);
        }
        {
            let mut data = self.project_data.write().unwrap();
            data.databases.remove(id);
        }
    }
}
