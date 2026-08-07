use super::{
    NodeProtocol, ParameterConstraint, ParameterEditorSpec, ParameterKey, ParameterValues,
    TypeExpr, TypeId, TypedValue, Value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocatedParameterIssue {
    pub key: ParameterKey,
    pub kind: ParameterIssueKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterIssueKind {
    Unknown,
    Required,
    InvalidType,
    Constraint,
    InvalidNominal(Box<str>),
    InvalidResourceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralValidationIssue {
    MalformedWire,
    DeclaredTypeMismatch,
    ValueTypeMismatch,
}

pub fn validate_typed_literal(
    wire: &serde_json::Value,
    declared_type: &TypeExpr,
) -> Result<TypedValue, LiteralValidationIssue> {
    let decoded = serde_json::from_value::<TypedValue>(wire.clone())
        .map_err(|_| LiteralValidationIssue::MalformedWire)?;
    if !type_expr_accepts(declared_type, &decoded.value_type) {
        return Err(LiteralValidationIssue::DeclaredTypeMismatch);
    }
    if !protocol_value_matches_type(&decoded.value, &decoded.value_type) {
        return Err(LiteralValidationIssue::ValueTypeMismatch);
    }
    Ok(decoded)
}

fn type_expr_accepts(declared: &TypeExpr, actual: &TypeExpr) -> bool {
    match declared {
        TypeExpr::Unknown | TypeExpr::Generic(_) => true,
        TypeExpr::Union(options) => options
            .iter()
            .any(|option| type_expr_accepts(option, actual)),
        _ => declared == actual,
    }
}

fn protocol_value_matches_type(value: &Value, value_type: &TypeExpr) -> bool {
    match value_type {
        TypeExpr::Concrete(type_id) => match type_id.as_str() {
            "core.bool" => matches!(value, Value::Bool(_)),
            "core.int64" => matches!(value, Value::Integer(_)),
            "core.float64" => matches!(value, Value::Integer(_) | Value::Decimal(_)),
            "core.string" => matches!(value, Value::String(_)),
            _ => true,
        },
        TypeExpr::Union(options) => options
            .iter()
            .any(|option| protocol_value_matches_type(value, option)),
        TypeExpr::Unknown | TypeExpr::Generic(_) | TypeExpr::Applied { .. } => true,
    }
}

pub trait NominalParameterValidator {
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>>;
}

impl<F> NominalParameterValidator for F
where
    F: Fn(&TypeId, &serde_json::Value) -> Option<Result<(), String>>,
{
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self(type_id, value)
    }
}

pub fn validate_parameter_values(
    protocol: &NodeProtocol,
    values: &ParameterValues,
    nominal: &impl NominalParameterValidator,
) -> Vec<LocatedParameterIssue> {
    let mut issues = Vec::new();
    for key in values.keys() {
        if !protocol
            .parameters
            .parameters
            .iter()
            .any(|spec| &spec.key == key)
        {
            issues.push(issue(key, ParameterIssueKind::Unknown));
        }
    }
    for spec in protocol.parameters.parameters.iter() {
        let Some(value) = values.get(&spec.key) else {
            if spec.default_value.is_none()
                && spec.constraints.contains(&ParameterConstraint::Required)
            {
                issues.push(issue(&spec.key, ParameterIssueKind::Required));
            }
            continue;
        };
        if spec.constraints.contains(&ParameterConstraint::Required) && value.is_null() {
            issues.push(issue(&spec.key, ParameterIssueKind::Required));
            continue;
        }
        if !parameter_value_matches_type(value, &spec.value_type) {
            issues.push(issue(&spec.key, ParameterIssueKind::InvalidType));
            continue;
        }
        if spec
            .constraints
            .iter()
            .any(|constraint| !parameter_constraint_matches(value, constraint))
        {
            issues.push(issue(&spec.key, ParameterIssueKind::Constraint));
            continue;
        }
        if spec.editor == ParameterEditorSpec::Resource
            && !value.as_str().is_some_and(valid_opaque_resource_id)
        {
            issues.push(issue(&spec.key, ParameterIssueKind::InvalidResourceId));
            continue;
        }
        if let TypeExpr::Concrete(type_id) = &spec.value_type
            && let Some(Err(detail)) = nominal.validate_nominal_parameter(type_id, value)
        {
            issues.push(issue(
                &spec.key,
                ParameterIssueKind::InvalidNominal(detail.into()),
            ));
        }
    }
    issues
}

fn issue(key: &ParameterKey, kind: ParameterIssueKind) -> LocatedParameterIssue {
    LocatedParameterIssue {
        key: key.clone(),
        kind,
    }
}

fn valid_opaque_resource_id(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn parameter_value_matches_type(value: &serde_json::Value, expected: &TypeExpr) -> bool {
    match expected {
        TypeExpr::Concrete(id) => match id.as_str() {
            "core.bool" => value.is_boolean(),
            "core.int64" => value.as_i64().is_some(),
            "core.float64" => value.is_number(),
            "core.string" => value.is_string(),
            _ => true,
        },
        TypeExpr::Union(options) => options
            .iter()
            .any(|option| parameter_value_matches_type(value, option)),
        TypeExpr::Generic(_) | TypeExpr::Applied { .. } | TypeExpr::Unknown => true,
    }
}

fn parameter_constraint_matches(
    value: &serde_json::Value,
    constraint: &ParameterConstraint,
) -> bool {
    match constraint {
        ParameterConstraint::Required => !value.is_null(),
        ParameterConstraint::OneOf(options) => options
            .iter()
            .any(|option| protocol_value_matches_json(option, value)),
        ParameterConstraint::IntegerRange { min, max } => value.as_i64().is_some_and(|value| {
            min.is_none_or(|minimum| value >= minimum) && max.is_none_or(|maximum| value <= maximum)
        }),
        ParameterConstraint::Length { min, max } => parameter_length(value).is_some_and(|length| {
            min.is_none_or(|minimum| length >= minimum as usize)
                && max.is_none_or(|maximum| length <= maximum as usize)
        }),
    }
}

fn parameter_length(value: &serde_json::Value) -> Option<usize> {
    match value {
        serde_json::Value::String(value) => Some(value.chars().count()),
        serde_json::Value::Array(value) => Some(value.len()),
        serde_json::Value::Object(value) => Some(value.len()),
        _ => None,
    }
}

fn protocol_value_matches_json(expected: &Value, actual: &serde_json::Value) -> bool {
    match (expected, actual) {
        (Value::Null, serde_json::Value::Null) => true,
        (Value::Bool(expected), serde_json::Value::Bool(actual)) => expected == actual,
        (Value::Integer(expected), actual) => actual.as_i64() == Some(*expected),
        (Value::Unsigned(expected), actual) => actual.as_u64() == Some(*expected),
        (Value::Decimal(expected), serde_json::Value::String(actual)) => {
            expected.as_str() == actual
        }
        (Value::String(expected), serde_json::Value::String(actual)) => expected.as_ref() == actual,
        (Value::Bytes(expected), serde_json::Value::Array(actual)) => actual
            .iter()
            .map(serde_json::Value::as_u64)
            .eq(expected.iter().map(|byte| Some(u64::from(*byte)))),
        (Value::List(expected), serde_json::Value::Array(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| protocol_value_matches_json(expected, actual))
        }
        (Value::Object(expected), serde_json::Value::Object(actual)) => {
            expected.len() == actual.len()
                && expected.iter().all(|(key, expected)| {
                    actual
                        .get(key.as_ref())
                        .is_some_and(|actual| protocol_value_matches_json(expected, actual))
                })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::protocol::{TypedValue, Value};

    #[test]
    fn typed_literal_rejects_value_that_disagrees_with_its_declared_type() {
        let declared = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        let wire = serde_json::to_value(TypedValue {
            value_type: declared.clone(),
            value: Value::String("not-an-integer".into()),
        })
        .unwrap();

        assert!(matches!(
            validate_typed_literal(&wire, &declared),
            Err(LiteralValidationIssue::ValueTypeMismatch)
        ));
    }

    #[test]
    fn typed_literal_rejects_type_that_disagrees_with_the_port() {
        let declared = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        let wire = serde_json::to_value(TypedValue {
            value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
            value: Value::String("value".into()),
        })
        .unwrap();

        assert!(matches!(
            validate_typed_literal(&wire, &declared),
            Err(LiteralValidationIssue::DeclaredTypeMismatch)
        ));
    }
}
