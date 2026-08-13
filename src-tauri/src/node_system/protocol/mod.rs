#![deny(unused_must_use)]

mod data_series;
pub mod dataframe;
mod identity;
mod model;
mod parameter;
mod types;
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
pub(crate) use model::validate_execution;
pub use model::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy,
    ExecutionSemantics, InputBindingSpec, InputConsumption, LiteralPolicy, ManagedNodeRole,
    NodeCatalogProtocol, NodeInstanceDisplaySpec, NodeInterfaceProtocol, NodeProtocol, NodeScope,
    OutputProduction, PortDirection, PortEditorSpec, PortInstances, PortKind, PortMemberGroupSpec,
    PortSpec, ProtocolError, Purity, ResourceDisplayKind, RetryPolicy, RetryPolicyError,
};
pub use parameter::{
    DuplicateParameterKey, ParameterConstraint, ParameterEditorSpec, ParameterPresentation,
    ParameterSchema, ParameterSchemaError, ParameterSpec, ParameterValues,
};
pub use types::{
    ColumnRename, ColumnSelectionExpr, RelationalScalarType, RenameExpr, ResolvedSchemaFact,
    SchemaColumnRef, SchemaDependency, SchemaExpr, SchemaField, SchemaFieldLineage,
    TypeCompatibility, TypeConstraint, TypeExpr, TypeNormalizationError, TypeTerm,
    normalize_type_expr, type_exprs_compatibility,
};
pub use validation::{
    LiteralValidationIssue, LocatedParameterIssue, NominalParameterValidator, ParameterIssueKind,
    ParameterValidation, validate_and_prepare_parameter_values, validate_parameter_values,
    validate_typed_literal,
};
pub use value::{CanonicalDecimal, InvalidDecimal, ParameterValue, TypedValue, Value};
