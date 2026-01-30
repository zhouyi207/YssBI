//! Value 模块 - 基于 Polars/Arrow 的类型系统
//!
//! 提供统一的数据类型定义和转换功能，用于 BI 系统的数据处理

pub mod types;
pub mod conversions;
pub mod type_desc;
pub mod type_var;
pub mod type_constraint;
pub mod pin_type;
pub mod type_inference;

pub use types::{Value, ValueType};
pub use conversions::{from_json, to_json, from_polars, to_polars};
pub use type_desc::DataType;
pub use type_var::TypeVarId;
pub use type_constraint::TypeConstraint;
pub use pin_type::PinTypeDesc;
pub use type_inference::TypeInferenceContext;
