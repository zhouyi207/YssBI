//! Backend-neutral data values, ordered table literals and metadata contracts.
//!
//! Scalar values are shared by graph evaluation and table projections. They do not own
//! cell identities or editing state; database edits address rows and columns in a snapshot.

mod column_semantic;
mod conversion;
mod data_value;
mod value_type;

pub use column_semantic::{ColumnSemantic, NumericConstraints, SemanticType, SemanticValue};
pub use conversion::{
    ConversionDomain, ConversionMetadata, DatetimeRepresentation, NumericRepresentation,
    SemanticConversion, TemporalPrecision, TemporalType,
};
pub use data_value::{DataValue, DecimalLiteral, FilterLiteral, InvalidDecimal};
pub use value_type::{ValueType, ValueTypeParseError};
pub mod aggregation;
/// Nominal port type for a source relation together with its grouping keys.
pub const GROUPED_DATAFRAME_TYPE_ID: &str = "tabular.grouped_dataframe";
pub mod table;

mod tabular;
pub use tabular::{
    ComparisonOperation, FiniteFloat64, TabularColumn, TabularColumnName, TabularColumnsView,
    TabularContractError, TabularScalar, TabularSnapshot,
};
