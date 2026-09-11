//! Arrow batch conversion at the host's tabular boundary.
//!
//! RecordBatch and its exact Schema remain the storage/execution representation. JSON and
//! TabularSnapshot are bounded display/literal projections, never storage schema authorities.

mod edit_type;
mod scalar;
mod temporal;
pub use edit_type::editable_data_type;
mod schema;

pub use scalar::{array_to_json, json_to_array, normalize_batch_categories, to_record_batch};
pub use schema::{
    CategoryDomain, DatasetRowColumns, column_identity, data_type_name, database_schema_fact,
    dataset_row_columns, semantic_data_type, validate_storage_schema, with_column_metadata,
    with_row_columns, without_row_metadata,
};
pub use temporal::{
    cast_temporal_without_timezone, datetime_strings_without_timezone, timezone_free_array,
    timezone_free_batch, timezone_free_data_type, timezone_free_schema, timezone_free_text,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TabularArrowError {
    #[error("tabular value is incompatible with its exact type")]
    InvalidValue,
    #[error("tabular type is unsupported at this boundary")]
    UnsupportedType,
    #[error("tabular schema metadata is invalid")]
    InvalidSchema,
    #[error("tabular batch construction failed")]
    BuildFailed,
}

#[cfg(test)]
mod tests;
