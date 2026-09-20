use std::collections::{BTreeSet, HashMap};

use arrow::datatypes::{DataType, Field, Schema};
use serde::{Deserialize, Serialize};
use yss_data_contract::TabularColumnName;
use yss_data_contract::ValueType as GraphValueType;
use yss_database_contract::DatabaseId;
use yss_database_schema::{DatabaseColumnFact, DatabaseSchemaFact};

use crate::TabularArrowError;

const COLUMN_ID: &str = "yssbi.column_id";
const CATEGORIES: &str = "yssbi.categories";
const ROW_ID_COLUMN: &str = "yssbi.row_id_column";
const DISPLAY_ORDER_COLUMN: &str = "yssbi.display_order_column";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetRowColumns {
    pub row_id: String,
    pub display_order: String,
}

/// Catalog schemas, unlike arbitrary imported file schemas, explicitly identify stable rows
/// and their independently editable display order. Neither is inferred from a batch offset.
pub fn with_row_columns(
    schema: Schema,
    row_id: &str,
    display_order: &str,
) -> Result<Schema, TabularArrowError> {
    let mut metadata = schema.metadata().clone();
    metadata.insert(ROW_ID_COLUMN.into(), row_id.into());
    metadata.insert(DISPLAY_ORDER_COLUMN.into(), display_order.into());
    let schema = schema.with_metadata(metadata);
    dataset_row_columns(&schema)?;
    Ok(schema)
}

pub fn dataset_row_columns(
    schema: &Schema,
) -> Result<Option<DatasetRowColumns>, TabularArrowError> {
    match (
        schema.metadata().get(ROW_ID_COLUMN),
        schema.metadata().get(DISPLAY_ORDER_COLUMN),
    ) {
        (None, None) => Ok(None),
        (Some(row_id), Some(display_order)) if row_id != display_order => {
            let row = schema
                .field_with_name(row_id)
                .map_err(|_| TabularArrowError::InvalidSchema)?;
            let order = schema
                .field_with_name(display_order)
                .map_err(|_| TabularArrowError::InvalidSchema)?;
            if row.data_type() != &DataType::Int64
                || row.is_nullable()
                || order.data_type() != &DataType::Utf8
                || order.is_nullable()
            {
                return Err(TabularArrowError::InvalidSchema);
            }
            Ok(Some(DatasetRowColumns {
                row_id: row_id.clone(),
                display_order: display_order.clone(),
            }))
        }
        _ => Err(TabularArrowError::InvalidSchema),
    }
}

pub fn without_row_metadata(schema: &Schema) -> Schema {
    let mut metadata = schema.metadata().clone();
    metadata.remove(ROW_ID_COLUMN);
    metadata.remove(DISPLAY_ORDER_COLUMN);
    schema.clone().with_metadata(metadata)
}

/// Labels, rather than per-batch dictionary keys, identify categories across files/batches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryDomain {
    pub labels: Vec<String>,
    pub ordered: bool,
}

impl CategoryDomain {
    pub fn from_field(field: &Field) -> Result<Option<Self>, TabularArrowError> {
        field
            .metadata()
            .get(CATEGORIES)
            .map_or(Ok(None), |encoded| {
                let domain: Self =
                    serde_json::from_str(encoded).map_err(|_| TabularArrowError::InvalidSchema)?;
                domain.validate(field.data_type())?;
                Ok(Some(domain))
            })
    }

    fn validate(&self, dtype: &DataType) -> Result<(), TabularArrowError> {
        let unique: BTreeSet<_> = self.labels.iter().collect();
        if unique.len() != self.labels.len()
            || !matches!(dtype, DataType::Dictionary(_, value) if matches!(value.as_ref(), DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View))
        {
            return Err(TabularArrowError::InvalidSchema);
        }
        Ok(())
    }
}

/// Attach identity once on import. Renames copy metadata and therefore retain column identity.
pub fn with_column_metadata(
    field: Field,
    identity: &str,
    categories: Option<&CategoryDomain>,
) -> Result<Field, TabularArrowError> {
    if identity.trim().is_empty() {
        return Err(TabularArrowError::InvalidSchema);
    }
    let mut metadata: HashMap<_, _> = field.metadata().clone();
    metadata.insert(COLUMN_ID.into(), identity.into());
    if let Some(categories) = categories {
        categories.validate(field.data_type())?;
        metadata.insert(
            CATEGORIES.into(),
            serde_json::to_string(categories).map_err(|_| TabularArrowError::InvalidSchema)?,
        );
    } else {
        metadata.remove(CATEGORIES);
    }
    Ok(field.with_metadata(metadata))
}

pub fn column_identity(field: &Field) -> Result<&str, TabularArrowError> {
    field
        .metadata()
        .get(COLUMN_ID)
        .map(String::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or(TabularArrowError::InvalidSchema)
}

pub fn validate_storage_schema(schema: &Schema) -> Result<(), TabularArrowError> {
    dataset_row_columns(schema)?;
    let mut names = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for field in schema.fields() {
        if field.name().trim().is_empty()
            || !names.insert(field.name())
            || !identities.insert(column_identity(field)?)
        {
            return Err(TabularArrowError::InvalidSchema);
        }
        CategoryDomain::from_field(field)?;
        crate::column_semantic(field)?;
    }
    Ok(())
}

pub fn data_type_name(dtype: &DataType) -> String {
    let dtype = crate::timezone_free_data_type(dtype);
    match &dtype {
        DataType::Boolean => "Boolean".into(),
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => "String".into(),
        DataType::Date32 | DataType::Date64 => "Date".into(),
        DataType::Timestamp(unit, _) => {
            let unit = match unit {
                arrow::datatypes::TimeUnit::Second => "s",
                arrow::datatypes::TimeUnit::Millisecond => "ms",
                arrow::datatypes::TimeUnit::Microsecond => "us",
                arrow::datatypes::TimeUnit::Nanosecond => "ns",
            };
            format!("Datetime({unit})")
        }
        DataType::Time32(_) | DataType::Time64(_) => "Time".into(),
        DataType::Decimal32(p, s)
        | DataType::Decimal64(p, s)
        | DataType::Decimal128(p, s)
        | DataType::Decimal256(p, s) => format!("Decimal({p}, {s})"),
        DataType::Dictionary(_, _) => "Categorical".into(),
        dtype => dtype.to_string(),
    }
}

pub fn physical_type_name(dtype: &DataType) -> String {
    match dtype {
        DataType::Boolean => "Bool".into(),
        DataType::Date32 => "Date".into(),
        DataType::Timestamp(_, _) => data_type_name(dtype),
        _ => dtype.to_string(),
    }
}

pub fn database_schema_fact(
    database: &DatabaseId,
    schema: &Schema,
) -> Result<DatabaseSchemaFact, TabularArrowError> {
    let row_columns = dataset_row_columns(schema)?;
    let columns = schema
        .fields()
        .iter()
        .filter(|field| {
            row_columns.as_ref().is_none_or(|rows| {
                field.name() != &rows.row_id && field.name() != &rows.display_order
            })
        })
        .map(|field| {
            let semantic = crate::column_semantic(field)?;
            Ok(DatabaseColumnFact::new(
                TabularColumnName::try_from(field.name().as_str())
                    .map_err(|_| TabularArrowError::InvalidSchema)?,
                GraphValueType::Scalar(semantic.kind),
                field.is_nullable(),
            )
            .with_display_type(data_type_name(field.data_type()))
            .with_physical_type(physical_type_name(field.data_type()))
            .with_semantic(semantic))
        })
        .collect::<Result<Box<[_]>, _>>()?;
    Ok(DatabaseSchemaFact::from_columns(
        database.clone(),
        0,
        0,
        columns,
    ))
}
