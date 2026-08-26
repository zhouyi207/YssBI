//! Persisted data value and metadata contracts.

mod data_type;
mod data_value;

pub use data_type::{DataType, DataTypeParseError};
pub use data_value::{CategoricalRole, DataSeriesValue, DataValue, DummyInfo, TimeSeriesState};
