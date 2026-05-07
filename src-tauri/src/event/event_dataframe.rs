use crate::schema::ColumnInfoDTO;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventDataframe {
    #[serde(rename_all = "camelCase")]
    DataFrameCreated { id: String },
    #[serde(rename_all = "camelCase")]
    DataFrameDeleted { id: String },
    /// 异步物化（SQL / Excel 等非真·lazy 引擎）完成后回填 schema。
    /// `error` 与 `columns/rowCount` 互斥：失败时只填错误，前端展示提示。
    #[serde(rename_all = "camelCase")]
    DataFrameSchemaUpdated {
        id: String,
        columns: Vec<ColumnInfoDTO>,
        row_count: usize,
        column_count: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}
