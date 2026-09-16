use serde::{Deserialize, Serialize};
use yss_data_contract::ValueType;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSchema {
    pub columns: Vec<ColumnSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: ValueType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<yss_data_contract::ColumnSemantic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_type: Option<String>,
}
