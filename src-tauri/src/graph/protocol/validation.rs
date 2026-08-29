use super::{
    CanonicalDecimal, NodeProtocol, ParameterConstraint, ParameterEditorSpec, ParameterKey,
    ParameterValues, TypeExpr, TypeId, TypedValue, Value,
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
    nominal: &impl NominalParameterValidator,
) -> Result<TypedValue, LiteralValidationIssue> {
    let decoded = serde_json::from_value::<TypedValue>(wire.clone())
        .map_err(|_| LiteralValidationIssue::MalformedWire)?;
    validate_decoded_literal(decoded, declared_type, nominal)
}

pub fn normalize_json_literal(
    raw: &serde_json::Value,
    declared_type: &TypeExpr,
    nominal: &impl NominalParameterValidator,
) -> Result<TypedValue, LiteralValidationIssue> {
    let value = json_literal_to_protocol_value(raw, declared_type)?;
    validate_decoded_literal(
        TypedValue {
            value_type: declared_type.clone(),
            value,
        },
        declared_type,
        nominal,
    )
}

fn validate_decoded_literal(
    decoded: TypedValue,
    declared_type: &TypeExpr,
    nominal: &impl NominalParameterValidator,
) -> Result<TypedValue, LiteralValidationIssue> {
    if !type_expr_accepts(declared_type, &decoded.value_type) {
        return Err(LiteralValidationIssue::DeclaredTypeMismatch);
    }
    if !protocol_value_matches_type(&decoded.value, &decoded.value_type, nominal) {
        return Err(LiteralValidationIssue::ValueTypeMismatch);
    }
    Ok(decoded)
}

fn json_literal_to_protocol_value(
    raw: &serde_json::Value,
    declared_type: &TypeExpr,
) -> Result<Value, LiteralValidationIssue> {
    if matches!(declared_type, TypeExpr::Concrete(type_id) if type_id.as_str() == "core.bytes") {
        let bytes = raw
            .as_array()
            .ok_or(LiteralValidationIssue::ValueTypeMismatch)?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or(LiteralValidationIssue::ValueTypeMismatch)
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Value::Bytes(bytes));
    }

    match raw {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(value) => Ok(Value::Bool(*value)),
        serde_json::Value::Number(value) if value.is_i64() => {
            Ok(Value::Integer(value.as_i64().expect("checked i64")))
        }
        serde_json::Value::Number(value) if value.is_u64() => {
            Ok(Value::Unsigned(value.as_u64().expect("checked u64")))
        }
        serde_json::Value::Number(value) => CanonicalDecimal::new(value.to_string())
            .map(Value::Decimal)
            .map_err(|_| LiteralValidationIssue::MalformedWire),
        serde_json::Value::String(value) => Ok(Value::String(value.as_str().into())),
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| json_literal_to_protocol_value(value, &TypeExpr::Unknown))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.as_str().into(),
                    json_literal_to_protocol_value(value, &TypeExpr::Unknown)?,
                ))
            })
            .collect::<Result<_, LiteralValidationIssue>>()
            .map(Value::Object),
    }
}

fn type_expr_accepts(declared: &TypeExpr, actual: &TypeExpr) -> bool {
    match (declared, actual) {
        (TypeExpr::Concrete(expected), TypeExpr::Concrete(actual)) => expected == actual,
        (
            TypeExpr::Applied {
                constructor: expected,
                arguments: expected_arguments,
            },
            TypeExpr::Applied {
                constructor: actual,
                arguments: actual_arguments,
            },
        ) => {
            expected == actual
                && expected_arguments.len() == actual_arguments.len()
                && expected_arguments
                    .iter()
                    .zip(actual_arguments)
                    .all(|(expected, actual)| type_expr_accepts(expected, actual))
        }
        (TypeExpr::Union(options), TypeExpr::Union(actual_options)) => {
            !actual_options.is_empty()
                && actual_options.iter().all(|actual| {
                    options
                        .iter()
                        .any(|option| type_expr_accepts(option, actual))
                })
        }
        (TypeExpr::Union(options), actual) => options
            .iter()
            .any(|option| type_expr_accepts(option, actual)),
        (TypeExpr::Generic(_) | TypeExpr::Unknown, _)
        | (_, TypeExpr::Generic(_) | TypeExpr::Unknown | TypeExpr::Union(_))
        | (TypeExpr::Concrete(_), TypeExpr::Applied { .. })
        | (TypeExpr::Applied { .. }, TypeExpr::Concrete(_)) => false,
    }
}

fn protocol_value_matches_type(
    value: &Value,
    value_type: &TypeExpr,
    nominal: &impl NominalParameterValidator,
) -> bool {
    match value_type {
        TypeExpr::Concrete(type_id) => match type_id.as_str() {
            "core.bool" => matches!(value, Value::Bool(_)),
            "core.int64" => matches!(value, Value::Integer(_)),
            "core.float64" => matches!(value, Value::Integer(_) | Value::Decimal(_)),
            "core.string" => matches!(value, Value::String(_)),
            "core.bytes" => matches!(value, Value::Bytes(_)),
            "core.object" => matches!(value, Value::Object(_)),
            _ => nominal
                .validate_nominal_parameter(type_id, &protocol_value_to_json(value))
                .is_some_and(|result| result.is_ok()),
        },
        TypeExpr::Applied {
            constructor,
            arguments,
        } => match (constructor.as_str(), arguments.as_slice(), value) {
            (
                "core.list" | "core.array" | "core.data_series",
                [element_type],
                Value::List(values),
            ) => values
                .iter()
                .all(|value| protocol_value_matches_type(value, element_type, nominal)),
            ("core.map" | "core.struct" | "core.object", [value_type], Value::Object(values)) => {
                values
                    .values()
                    .all(|value| protocol_value_matches_type(value, value_type, nominal))
            }
            _ => false,
        },
        TypeExpr::Union(options) => options
            .iter()
            .any(|option| protocol_value_matches_type(value, option, nominal)),
        TypeExpr::Unknown | TypeExpr::Generic(_) => false,
    }
}

pub fn protocol_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => (*value).into(),
        Value::Integer(value) => (*value).into(),
        Value::Unsigned(value) => (*value).into(),
        Value::Decimal(value) => value.as_str().into(),
        Value::String(value) => value.as_ref().into(),
        Value::Bytes(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| serde_json::Value::from(*value))
                .collect(),
        ),
        Value::List(values) => values.iter().map(protocol_value_to_json).collect(),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| (key.to_string(), protocol_value_to_json(value)))
            .collect(),
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
    F: for<'a, 'b> Fn(&'a TypeId, &'b serde_json::Value) -> Option<Result<(), String>>,
{
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self(type_id, value)
    }
}

#[derive(Debug)]
pub struct ParameterValidation<T> {
    pub issues: Vec<LocatedParameterIssue>,
    pub prepared_nominal: std::collections::BTreeMap<ParameterKey, T>,
}

pub fn validate_parameter_values(
    protocol: &NodeProtocol,
    values: &ParameterValues,
    nominal: &impl NominalParameterValidator,
) -> Vec<LocatedParameterIssue> {
    validate_and_prepare_parameter_values(protocol, values, |type_id, value| {
        nominal.validate_nominal_parameter(type_id, value)
    })
    .issues
}

pub fn validate_and_prepare_parameter_values<T>(
    protocol: &NodeProtocol,
    values: &ParameterValues,
    prepare_nominal: impl Fn(&TypeId, &serde_json::Value) -> Option<Result<T, String>>,
) -> ParameterValidation<T> {
    let mut issues = Vec::new();
    let mut prepared_nominal = std::collections::BTreeMap::new();
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
        if matches!(spec.editor, ParameterEditorSpec::Resource { .. })
            && !value.as_str().is_some_and(valid_opaque_resource_id)
        {
            issues.push(issue(&spec.key, ParameterIssueKind::InvalidResourceId));
            continue;
        }
        if let TypeExpr::Concrete(type_id) = &spec.value_type {
            match prepare_nominal(type_id, value) {
                Some(Ok(prepared)) => {
                    prepared_nominal.insert(spec.key.clone(), prepared);
                }
                Some(Err(detail)) => issues.push(issue(
                    &spec.key,
                    ParameterIssueKind::InvalidNominal(detail.into()),
                )),
                None => {}
            }
        }
    }
    ParameterValidation {
        issues,
        prepared_nominal,
    }
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
    use crate::graph::protocol::{TypeConstructorId, TypeParameterId, TypedValue, Value};
    use std::collections::BTreeMap;

    struct NoNominalValidator;

    impl NominalParameterValidator for NoNominalValidator {
        fn validate_nominal_parameter(
            &self,
            _: &TypeId,
            _: &serde_json::Value,
        ) -> Option<Result<(), String>> {
            None
        }
    }

    #[test]
    fn typed_literal_rejects_value_that_disagrees_with_its_declared_type() {
        let declared = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        let wire = serde_json::to_value(TypedValue {
            value_type: declared.clone(),
            value: Value::String("not-an-integer".into()),
        })
        .unwrap();

        assert!(matches!(
            validate_typed_literal(&wire, &declared, &NoNominalValidator),
            Err(LiteralValidationIssue::ValueTypeMismatch)
        ));
    }

    #[test]
    fn typed_literal_rejects_nested_list_element_mismatch() {
        let list = TypeExpr::Applied {
            constructor: crate::graph::protocol::TypeConstructorId::new("core.list").unwrap(),
            arguments: vec![TypeExpr::Concrete(TypeId::new("core.int64").unwrap())],
        };
        let wire = serde_json::to_value(TypedValue {
            value_type: list.clone(),
            value: Value::List(vec![Value::Integer(1), Value::String("wrong".into())]),
        })
        .unwrap();

        assert!(matches!(
            validate_typed_literal(&wire, &list, &NoNominalValidator),
            Err(LiteralValidationIssue::ValueTypeMismatch)
        ));
    }

    #[test]
    fn typed_literal_rejects_unverifiable_concrete_type() {
        let nominal = TypeExpr::Concrete(TypeId::new("acme.unknown").unwrap());
        let wire = serde_json::to_value(TypedValue {
            value_type: nominal.clone(),
            value: Value::Object(Default::default()),
        })
        .unwrap();

        assert!(matches!(
            validate_typed_literal(&wire, &nominal, &NoNominalValidator),
            Err(LiteralValidationIssue::ValueTypeMismatch)
        ));
    }

    #[test]
    fn typed_literal_accepts_bytes_objects_and_recursive_collections() {
        let int64 = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        let cases = [
            (
                TypeExpr::Concrete(TypeId::new("core.bytes").unwrap()),
                Value::Bytes(vec![0, 127, 255]),
            ),
            (
                TypeExpr::Concrete(TypeId::new("core.object").unwrap()),
                Value::Object(BTreeMap::from([("value".into(), Value::Integer(1))])),
            ),
            (
                TypeExpr::Applied {
                    constructor: TypeConstructorId::new("core.list").unwrap(),
                    arguments: vec![int64.clone()],
                },
                Value::List(vec![Value::Integer(1), Value::Integer(2)]),
            ),
            (
                TypeExpr::Applied {
                    constructor: TypeConstructorId::new("core.struct").unwrap(),
                    arguments: vec![int64],
                },
                Value::Object(BTreeMap::from([
                    ("first".into(), Value::Integer(1)),
                    ("second".into(), Value::Integer(2)),
                ])),
            ),
        ];

        for (value_type, value) in cases {
            let wire = serde_json::to_value(TypedValue {
                value_type: value_type.clone(),
                value,
            })
            .unwrap();
            assert_eq!(
                validate_typed_literal(&wire, &value_type, &NoNominalValidator),
                serde_json::from_value(wire).map_err(|_| LiteralValidationIssue::MalformedWire)
            );
        }
    }

    #[test]
    fn typed_literal_accepts_union_value_type_when_every_option_fits_the_port_oneof() {
        let oneof = TypeExpr::Union(vec![
            TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
        ]);
        let wire = serde_json::to_value(TypedValue {
            value_type: oneof.clone(),
            value: Value::String("selected-option".into()),
        })
        .unwrap();

        assert!(validate_typed_literal(&wire, &oneof, &NoNominalValidator).is_ok());
    }

    #[test]
    fn typed_literal_accepts_registered_nominal_value() {
        struct AcceptNominal;
        impl NominalParameterValidator for AcceptNominal {
            fn validate_nominal_parameter(
                &self,
                type_id: &TypeId,
                value: &serde_json::Value,
            ) -> Option<Result<(), String>> {
                (type_id.as_str() == "acme.count").then(|| {
                    value
                        .as_u64()
                        .filter(|value| *value <= 10)
                        .map(|_| ())
                        .ok_or_else(|| "count out of range".to_owned())
                })
            }
        }

        let nominal = TypeExpr::Concrete(TypeId::new("acme.count").unwrap());
        let wire = serde_json::to_value(TypedValue {
            value_type: nominal.clone(),
            value: Value::Unsigned(7),
        })
        .unwrap();

        assert!(validate_typed_literal(&wire, &nominal, &AcceptNominal).is_ok());
    }

    #[test]
    fn typed_literal_fails_closed_for_unknown_type_shapes() {
        let int64 = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
        let cases = [
            TypeExpr::Applied {
                constructor: TypeConstructorId::new("acme.unknown").unwrap(),
                arguments: vec![int64],
            },
            TypeExpr::Generic(TypeParameterId::new("item").unwrap()),
            TypeExpr::Unknown,
        ];

        for value_type in cases {
            let wire = serde_json::to_value(TypedValue {
                value_type: value_type.clone(),
                value: Value::Integer(1),
            })
            .unwrap();
            assert!(validate_typed_literal(&wire, &value_type, &NoNominalValidator).is_err());
        }
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
            validate_typed_literal(&wire, &declared, &NoNominalValidator),
            Err(LiteralValidationIssue::DeclaredTypeMismatch)
        ));
    }
}
