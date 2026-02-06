use super::DatabaseEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDecl {
    pub id: String,             // 逻辑名，如 "MainDB"
    pub engine: DatabaseEngine, // SQLite / Postgres
    pub schema_version: u32,
    pub required: bool,
}


