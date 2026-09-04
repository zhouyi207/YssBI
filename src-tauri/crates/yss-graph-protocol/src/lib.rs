#![deny(unused_must_use)]

mod data_series;
pub mod dataframe;
mod identity;
mod model;
mod parameter;
mod types;
mod typing;
pub mod validation;

#[cfg(test)]
mod tests;
mod value;

pub use data_series::{
    DATA_SERIES_CONSTRUCTOR_ID, NUMERIC_TYPE_CLASS_ID, data_series_type, numeric_data_series_type,
};
pub use identity::{
    I18nKey, IconId, InterfaceResolverId, InvalidSemanticId, NodeCategoryId, NodeStyleId,
    NodeTypeId, ParameterKey, PortKey, ProviderId, SchemaResolverId, TypeClassId,
    TypeConstructorId, TypeId, TypeParameterId,
};
pub use model::{
    CachePolicy, ConnectionsPerPort, Determinism, ExecutionSemantics, InputBindingSpec,
    InputConsumption, LiteralPolicy, ManagedNodeRole, NodeCatalogProtocol, NodeInstanceDisplaySpec,
    NodeInterfaceProtocol, NodeProtocol, NodeScope, OutputProduction, PortCardinality,
    PortDirection, PortEditorSpec, PortMemberGroupSpec, PortSpec, ProtocolError,
    ResourceDisplayKind, validate_execution,
};
pub use parameter::{
    DuplicateParameterKey, ParameterConstraint, ParameterEditorSpec, ParameterPresentation,
    ParameterSchema, ParameterSchemaError, ParameterSpec, ParameterValues,
};
pub use types::{
    ColumnRename, ColumnSelectionExpr, RelationalScalarType, RenameExpr, ResolvedSchemaFact,
    SchemaColumnRef, SchemaDependency, SchemaExpr, SchemaField, SchemaFieldLineage,
    TypeCompatibility, TypeExpr, TypeNormalizationError, normalize_type_expr,
    type_exprs_compatibility,
};
pub use typing::{
    InputCoercionKind, NodeTypingSpec, NumericPromotionRule, PortSelector, ResolvedType, ShapeRule,
    TypeConflict, TypeDomain, TypeState, TypeUnknownReason,
};
pub use validation::{
    LiteralValidationIssue, LocatedParameterIssue, ParameterIssueKind, ParameterValidation,
    TypeValidationContext, normalize_json_literal, protocol_value_to_json,
    validate_and_prepare_parameter_values, validate_parameter_values, validate_typed_literal,
    validate_typed_value,
};
pub use value::{CanonicalDecimal, InvalidDecimal, ParameterValue, TypedValue, Value};
