/// SQL engine enumeration
///
/// Defines the different SQL database engines supported by the system.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DatabaseEngineSql {
    Sqlite { auto_create: bool },
    Postgres { ssl: bool },
    Mysql { charset: String },
}
