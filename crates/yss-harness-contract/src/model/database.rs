//! Database business commands; the existing resource envelope owns concurrency.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::TabularScalar;

mod reads;
pub use reads::*;
mod profile;
pub use profile::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseResourceKind {
    Database,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseResourceRef {
    pub kind: DatabaseResourceKind,
    pub id: String,
}

impl DatabaseResourceRef {
    pub fn new(id: String) -> Self {
        Self {
            kind: DatabaseResourceKind::Database,
            id,
        }
    }
    pub fn resource(&self) -> ProjectResourceRef {
        ProjectResourceRef {
            kind: ProjectResourceKind::Database,
            id: self.id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InsertRowsInput {
    pub database: DatabaseResourceRef,
    /// Initial values keyed by existing column names. Omitted values become null.
    #[schemars(length(min = 1, max = 200))]
    pub rows: Vec<BTreeMap<String, TabularScalar>>,
    /// Insert before this stable row ID; omit to append. This is not a page position.
    #[serde(default)]
    pub before_row_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseCellEdit {
    pub row_id: i64,
    pub column: String,
    pub value: TabularScalar,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCellsInput {
    pub database: DatabaseResourceRef,
    /// Each rowId/column pair must occur only once. Null explicitly clears a cell.
    #[schemars(length(min = 1, max = 200))]
    pub cells: Vec<DatabaseCellEdit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteRowsInput {
    pub database: DatabaseResourceRef,
    /// Stable IDs returned by a database read or insertion, never display row numbers.
    #[schemars(length(min = 1, max = 200))]
    pub row_ids: Vec<i64>,
}

pub(crate) fn validate_cells(cells: &[DatabaseCellEdit]) -> Result<(), CapabilityContractError> {
    let mut seen = BTreeSet::new();
    for cell in cells {
        validate_resource_id("cells.column", &cell.column)?;
        if cell.row_id < 0 || !seen.insert((cell.row_id, &cell.column)) {
            return Err(CapabilityContractError::InvalidField("cells"));
        }
    }
    Ok(())
}

pub(crate) fn validate_row_ids(ids: &[i64]) -> Result<(), CapabilityContractError> {
    if ids.iter().any(|id| *id < 0) || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(CapabilityContractError::InvalidField("rowIds"));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseColumnDeclaration {
    pub name: String,
    /// Canonical physical type, e.g. Int64, Float64, Utf8, Bool, Date, Datetime(ms), Decimal(18,2).
    pub dtype: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseColumnRename {
    pub column: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseColumnCast {
    pub column: String,
    /// Canonical physical type, e.g. Int64, Float64, Utf8, Bool, Date, Datetime(ms), Decimal(18,2).
    pub dtype: String,
    /// Explicitly allow unrepresentable values to become null. Semantic constraints still apply.
    pub force: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseColumnMeaning {
    pub column: String,
    pub semantic: DatasetColumnSemantic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateColumnsInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub columns: Vec<DatabaseColumnDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameColumnsInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub columns: Vec<DatabaseColumnRename>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteColumnsInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CastColumnsInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub columns: Vec<DatabaseColumnCast>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetColumnSemanticsInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 200))]
    pub columns: Vec<DatabaseColumnMeaning>,
}

pub(crate) fn validate_column_names<'a>(
    names: impl Iterator<Item = &'a str>,
) -> Result<(), CapabilityContractError> {
    let mut seen = BTreeSet::new();
    for name in names {
        validate_resource_id("columns", name)?;
        if !seen.insert(name) {
            return Err(CapabilityContractError::InvalidField("columns"));
        }
    }
    Ok(())
}
