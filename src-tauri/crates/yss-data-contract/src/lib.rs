//! Canonical persisted data value and metadata contracts.

mod column_semantic;
mod conversion;
mod data_value;
mod value_type;

pub use column_semantic::{ColumnSemantic, NumericConstraints, SemanticType, SemanticValue};
pub use conversion::{
    ConversionDomain, ConversionMetadata, DatetimeRepresentation, NumericRepresentation,
    SemanticConversion, TemporalPrecision, TemporalType,
};
pub use data_value::{CategoricalRole, DataSeriesValue, DataValue, DummyInfo, TimeSeriesState};
pub use value_type::{ValueType, ValueTypeParseError};
pub mod table;
