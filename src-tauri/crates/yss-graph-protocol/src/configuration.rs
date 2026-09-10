use crate::{ParameterKey, ParameterSpec, Value, protocol_value_to_json};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

/// Fields for a configuration object owned by one node parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationSchema {
    pub fields: Box<[ConfigurationFieldSpec]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationFieldSpec {
    pub parameter: ParameterSpec,
    pub visible_when: Option<ConfigurationCondition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationCondition {
    pub key: ParameterKey,
    pub values: Box<[Value]>,
}

impl ConfigurationFieldSpec {
    pub fn is_visible(&self, values: &Map<String, JsonValue>) -> bool {
        self.visible_when.as_ref().is_none_or(|condition| {
            values.get(condition.key.as_str()).is_some_and(|actual| {
                condition
                    .values
                    .iter()
                    .any(|value| protocol_value_to_json(value) == *actual)
            })
        })
    }
}

impl ConfigurationSchema {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.fields.is_empty() {
            return Err("configuration requires fields");
        }
        let mut keys = std::collections::BTreeSet::new();
        for field in &self.fields {
            if !keys.insert(&field.parameter.key) {
                return Err("duplicate configuration field");
            }
            if !matches!(&field.parameter.value_type, crate::TypeExpr::Concrete(id)
                if matches!(id.as_str(), "core.bool" | "core.int64" | "core.float64" | "core.string"))
            {
                return Err("configuration fields require a scalar value type");
            }
            let default = field
                .parameter
                .default_value
                .as_ref()
                .ok_or("configuration fields require defaults")?;
            let default = normalize_number(
                protocol_value_to_json(&default.value),
                &field.parameter.value_type,
            );
            if !crate::validation::parameter_value_matches_type(
                &default,
                &field.parameter.value_type,
            ) || field.parameter.constraints.iter().any(|constraint| {
                !crate::validation::parameter_constraint_matches(&default, constraint)
            }) {
                return Err("configuration field default is invalid");
            }
            if let Some(condition) = &field.visible_when {
                let selector = self.fields.iter().find(|candidate| {
                    candidate.parameter.key == condition.key && candidate.visible_when.is_none()
                });
                let Some(selector) = selector else {
                    return Err("configuration condition requires an unconditional field");
                };
                if condition.values.is_empty()
                    || condition.values.iter().any(|value| {
                        let value = normalize_number(
                            protocol_value_to_json(value),
                            &selector.parameter.value_type,
                        );
                        !crate::validation::parameter_value_matches_type(
                            &value,
                            &selector.parameter.value_type,
                        ) || selector.parameter.constraints.iter().any(|constraint| {
                            !crate::validation::parameter_constraint_matches(&value, constraint)
                        })
                    })
                {
                    return Err("configuration condition contains an invalid selector value");
                }
            }
        }
        self.normalize_json(&JsonValue::Object(Map::new()))
            .map_err(|_| "configuration defaults are invalid")?;
        Ok(())
    }

    /// Projection can still show editable fields while an imported value is invalid.
    pub fn effective_values(&self, raw: &JsonValue) -> Map<String, JsonValue> {
        self.fields
            .iter()
            .filter_map(|field| {
                let parameter = &field.parameter;
                let value = raw.get(parameter.key.as_str()).cloned().or_else(|| {
                    parameter
                        .default_value
                        .as_ref()
                        .map(|value| protocol_value_to_json(&value.value))
                })?;
                Some((
                    parameter.key.to_string(),
                    normalize_number(value, &parameter.value_type),
                ))
            })
            .collect()
    }

    pub fn normalize_json(&self, raw: &JsonValue) -> Result<JsonValue, String> {
        let supplied = raw.as_object().ok_or("configuration must be an object")?;
        for key in supplied.keys() {
            if !self
                .fields
                .iter()
                .any(|field| field.parameter.key.as_str() == key)
            {
                return Err(format!("unknown configuration field '{key}'"));
            }
        }
        let values = self.effective_values(raw);
        let mut normalized = Map::new();
        for field in self.fields.iter().filter(|field| field.is_visible(&values)) {
            let parameter = &field.parameter;
            let value = values
                .get(parameter.key.as_str())
                .ok_or_else(|| format!("missing configuration field '{}'", parameter.key))?;
            if !crate::validation::parameter_value_matches_type(value, &parameter.value_type)
                || parameter.constraints.iter().any(|constraint| {
                    !crate::validation::parameter_constraint_matches(value, constraint)
                })
            {
                return Err(format!("invalid configuration field '{}'", parameter.key));
            }
            normalized.insert(parameter.key.to_string(), value.clone());
        }
        Ok(JsonValue::Object(normalized))
    }

    /// A stored parameter must already contain the active fields: validators do
    /// not rewrite the value that compilation will consume.
    pub fn validate_json(&self, raw: &JsonValue) -> Result<(), String> {
        let normalized = self.normalize_json(raw)?;
        if raw
            .as_object()
            .map(|value| value.keys().collect::<std::collections::BTreeSet<_>>())
            != normalized
                .as_object()
                .map(|value| value.keys().collect::<std::collections::BTreeSet<_>>())
        {
            return Err("configuration must contain exactly its active fields".into());
        }
        Ok(())
    }
}

fn normalize_number(value: JsonValue, value_type: &crate::TypeExpr) -> JsonValue {
    if matches!(value_type, crate::TypeExpr::Concrete(id) if id.as_str() == "core.float64")
        && let Some(number) = value
            .as_str()
            .and_then(|value| value.parse::<f64>().ok())
            .and_then(serde_json::Number::from_f64)
    {
        return JsonValue::Number(number);
    }
    value
}
