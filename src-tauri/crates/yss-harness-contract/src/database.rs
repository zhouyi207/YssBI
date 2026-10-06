//! Internal read envelope and bounded business facts; no database state is owned here.
use crate::{CapabilityId, DatasetColumnSchema, InspectionPage, ResourceVersion, model};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yss_data_contract::TabularScalar;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseReadRequest {
    pub input: model::DatabaseReadInput,
    pub version: Option<ResourceVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseReadResult {
    pub database: model::DatabaseResourceRef,
    pub version: ResourceVersion,
    pub content: DatabaseReadContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum DatabaseReadContent {
    Overview {
        name: String,
        row_count: usize,
        column_count: usize,
        dirty: bool,
        can_undo: bool,
        can_redo: bool,
    },
    Schema {
        columns: Vec<DatasetColumnSchema>,
        page: InspectionPage,
    },
    Profile {
        columns: Vec<String>,
        metrics: model::DatabaseProfileMetricSelection,
    },
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<TabularScalar>>,
        row_ids: Vec<i64>,
        page: InspectionPage,
    },
}

impl DatabaseReadContent {
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::Overview { .. } => CapabilityId::InspectDatabase,
            Self::Schema { .. } => CapabilityId::InspectDatabaseSchema,
            Self::Profile { .. } => CapabilityId::ProfileDatabase,
            Self::Rows { .. } => CapabilityId::ReadDatabaseRows,
        }
    }
}
