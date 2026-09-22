use super::DatabaseDeclDTO;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 分阶段加载：databases（第一步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDatabasesDTO {
    pub databases: HashMap<String, DatabaseDeclDTO>,
}
