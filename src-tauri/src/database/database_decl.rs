use super::DatabaseEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDecl {
    pub id: String,
    pub engine: DatabaseEngine,
    pub schema_version: u32,
    pub required: bool,
}
