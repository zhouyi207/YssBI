use serde::{Deserialize, Serialize};

/// Tabular data schema shared by database and result-source code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataSchema {
    pub columns: Vec<ColumnSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: crate::graph::DataType,
}
