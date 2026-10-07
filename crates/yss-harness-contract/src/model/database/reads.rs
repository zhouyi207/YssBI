//! Selected database reads. Owner baselines are added after model authorization.
use super::*;
use yss_data_contract::FilterLiteral;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDatabaseInput {
    pub database: DatabaseResourceRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectDatabaseSchemaInput {
    pub database: DatabaseResourceRef,
    /// Empty selects all columns in schema order, then applies the page.
    #[serde(default)]
    #[schemars(length(max = 100))]
    pub columns: Vec<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "schema_limit")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: usize,
}
fn schema_limit() -> usize {
    50
}
fn row_limit() -> usize {
    20
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseProfileMetric {
    Completeness,
    Statistics,
    Distribution,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileDatabaseInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 100))]
    pub columns: Vec<String>,
    #[schemars(length(min = 1, max = 3))]
    pub metrics: Vec<DatabaseProfileMetric>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseComparison {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    IsNull,
    IsNotNull,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseFilter {
    pub column: String,
    pub comparison: DatabaseComparison,
    /// Required for comparisons; omit for is_null/is_not_null. Exact numbers use the literal string format.
    #[serde(default)]
    pub value: Option<FilterLiteral>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseOrder {
    pub column: String,
    pub ascending: bool,
    pub nulls_first: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadDatabaseRowsInput {
    pub database: DatabaseResourceRef,
    #[schemars(length(min = 1, max = 100))]
    pub columns: Vec<String>,
    /// All predicates must match (AND). Filter and order columns need not be returned.
    #[serde(default)]
    #[schemars(length(max = 100))]
    pub filters: Vec<DatabaseFilter>,
    #[serde(default)]
    #[schemars(length(max = 100))]
    pub order: Vec<DatabaseOrder>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "row_limit")]
    #[schemars(range(min = 1, max = 1000))]
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "input",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DatabaseReadInput {
    Overview(InspectDatabaseInput),
    Schema(InspectDatabaseSchemaInput),
    Profile(ProfileDatabaseInput),
    Rows(ReadDatabaseRowsInput),
}

impl DatabaseReadInput {
    pub fn database(&self) -> &DatabaseResourceRef {
        match self {
            Self::Overview(v) => &v.database,
            Self::Schema(v) => &v.database,
            Self::Profile(v) => &v.database,
            Self::Rows(v) => &v.database,
        }
    }
    pub const fn capability_id(&self) -> CapabilityId {
        match self {
            Self::Overview(_) => CapabilityId::InspectDatabase,
            Self::Schema(_) => CapabilityId::InspectDatabaseSchema,
            Self::Profile(_) => CapabilityId::ProfileDatabase,
            Self::Rows(_) => CapabilityId::ReadDatabaseRows,
        }
    }
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        use CapabilityContractError::InvalidField;
        validate_resource_id("database.id", &self.database().id)?;
        let (columns, required) = match self {
            Self::Overview(_) => return Ok(()),
            Self::Schema(v) => {
                page_limit(v.limit, 100)?;
                (&v.columns, false)
            }
            Self::Profile(v) => {
                if v.metrics.is_empty()
                    || v.metrics.iter().collect::<BTreeSet<_>>().len() != v.metrics.len()
                {
                    return Err(InvalidField("metrics"));
                }
                (&v.columns, true)
            }
            Self::Rows(v) => {
                page_limit(v.limit, 1000)?;
                if v.filters.len() > 100 || v.order.len() > 100 {
                    return Err(InvalidField("filters/order"));
                }
                for filter in &v.filters {
                    validate_resource_id("filters.column", &filter.column)?;
                    let unary = matches!(
                        filter.comparison,
                        DatabaseComparison::IsNull | DatabaseComparison::IsNotNull
                    );
                    if unary == filter.value.is_some() {
                        return Err(InvalidField("filters.value"));
                    }
                }
                validate_column_names(v.order.iter().map(|value| value.column.as_str()))?;
                (&v.columns, true)
            }
        };
        if columns.len() > 100 || (required && columns.is_empty()) {
            return Err(InvalidField("columns"));
        }
        validate_column_names(columns.iter().map(String::as_str))
    }
}
fn page_limit(limit: usize, maximum: u16) -> Result<(), CapabilityContractError> {
    if limit == 0 || limit > usize::from(maximum) {
        Err(CapabilityContractError::InvalidLimit { maximum })
    } else {
        Ok(())
    }
}
