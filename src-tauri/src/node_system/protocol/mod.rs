#![deny(unused_must_use)]

pub mod dataframe;
mod identity;
mod model;
mod parameter;
mod types;
pub mod validation;
mod value;

pub use identity::{
    I18nKey, IconId, InterfaceResolverId, InvalidSemanticId, NodeCategoryId, NodeStyleId,
    NodeTypeId, ParameterKey, PortKey, ProviderId, SchemaResolverId, TypeClassId,
    TypeConstructorId, TypeId, TypeParameterId,
};
pub(crate) use model::validate_execution;
pub use model::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy,
    ExecutionSemantics, InputBindingSpec, InputConsumption, LiteralPolicy, ManagedNodeRole,
    NodeCatalogProtocol, NodeInterfaceProtocol, NodeProtocol, NodeScope, OutputProduction,
    PortDirection, PortEditorSpec, PortInstances, PortKind, PortMemberGroupSpec, PortSpec,
    ProtocolError, Purity, RetryPolicy, RetryPolicyError,
};
pub use parameter::{
    DuplicateParameterKey, ParameterConstraint, ParameterEditorSpec, ParameterSchema,
    ParameterSchemaError, ParameterSpec, ParameterValues,
};
pub use types::{
    ColumnRename, ColumnSelectionExpr, RelationalScalarType, RenameExpr, ResolvedSchemaFact,
    SchemaColumnRef, SchemaDependency, SchemaExpr, SchemaField, TypeConstraint, TypeExpr, TypeTerm,
};
pub use validation::{
    LiteralValidationIssue, LocatedParameterIssue, NominalParameterValidator, ParameterIssueKind,
    ParameterValidation, validate_and_prepare_parameter_values, validate_parameter_values,
    validate_typed_literal,
};
pub use value::{CanonicalDecimal, InvalidDecimal, ParameterValue, TypedValue, Value};
