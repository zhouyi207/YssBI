use arrow_array::{ArrayRef, RecordBatch};
use arrow_schema::SchemaRef;
use std::path::PathBuf;

/// Sparse values for one stable column identity. Presence in row_ids distinguishes an explicit
/// null from an unedited cell; values carry the column's exact Arrow type.
#[derive(Clone)]
pub struct DatasetColumnPatch {
    pub column_id: Box<str>,
    pub row_ids: Box<[i64]>,
    pub values: ArrayRef,
}

#[derive(Clone, Default)]
pub struct DatasetOverlay {
    pub columns: Box<[DatasetColumnPatch]>,
    pub inserted: Box<[RecordBatch]>,
    pub deleted: Box<[i64]>,
}

/// Immutable input to the relational adapter. Catalog mutation is not part of this port.
#[derive(Clone)]
pub struct DatasetRelationInput {
    pub base_schema: SchemaRef,
    pub schema: SchemaRef,
    /// Managed generation files are each ordered by the base schema's DisplayOrder,
    /// then RowId, ascending with nulls last. File list order need not be global row order.
    /// Arbitrary Parquet files without this guarantee use the generic relation boundary.
    pub files: Box<[PathBuf]>,
    pub overlay: DatasetOverlay,
}
