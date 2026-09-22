use super::{I18nKey, ParameterGroupKey, ParameterKey, ResourceDisplayKind, TypeExpr, TypedValue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use yss_data_contract::{DataValue, DecimalLiteral};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    pub key: ParameterKey,
    pub title_key: I18nKey,
    pub description_key: Option<I18nKey>,
    pub value_type: TypeExpr,
    pub default_value: Option<TypedValue>,
    pub constraints: Vec<ParameterConstraint>,
    pub editor: ParameterEditorSpec,
    pub presentation: ParameterPresentation,
    pub visible_when: Option<ParameterCondition>,
}

impl Parameter {
    /// Builder for trusted node declarations; registration validates the completed schema.
    pub fn number(key: &str) -> Self {
        Self {
            key: ParameterKey::new(key).expect("valid declared parameter key"),
            title_key: I18nKey::new(format!("parameters.{key}.title"))
                .expect("valid parameter title"),
            description_key: None,
            value_type: TypeExpr::Concrete("core.numeric".parse().expect("numeric type")),
            default_value: None,
            constraints: vec![],
            editor: ParameterEditorSpec::Number,
            presentation: ParameterPresentation::DetailPanel,
            visible_when: None,
        }
    }

    pub fn title(mut self, key: I18nKey) -> Self {
        self.title_key = key;
        self
    }

    pub fn float(mut self) -> Self {
        self.constraints
            .retain(|constraint| !matches!(constraint, ParameterConstraint::IntegerRange { .. }));
        self
    }

    pub fn int(mut self) -> Self {
        if !self
            .constraints
            .iter()
            .any(|constraint| matches!(constraint, ParameterConstraint::IntegerRange { .. }))
        {
            self.constraints.push(ParameterConstraint::IntegerRange {
                min: None,
                max: None,
            });
        }
        self
    }

    pub fn positive(mut self) -> Self {
        self.constraints.push(ParameterConstraint::Positive);
        self
    }

    pub fn min(mut self, minimum: i64) -> Self {
        let Some(ParameterConstraint::IntegerRange { min, .. }) = self
            .constraints
            .iter_mut()
            .find(|constraint| matches!(constraint, ParameterConstraint::IntegerRange { .. }))
        else {
            panic!("integer minimum requires an int parameter");
        };
        *min = Some(minimum);
        self
    }

    pub fn default(mut self, value: impl ToString) -> Self {
        let text = value.to_string();
        let value = if self
            .constraints
            .iter()
            .any(|constraint| matches!(constraint, ParameterConstraint::IntegerRange { .. }))
        {
            DataValue::Integer(text.parse().expect("integer parameter default"))
        } else {
            DataValue::Decimal(DecimalLiteral::new(text).expect("finite numeric parameter default"))
        };
        self.default_value = Some(TypedValue {
            value_type: self.value_type.clone(),
            value,
        });
        self
    }

    pub fn when(mut self, condition: ParameterCondition) -> Self {
        self.visible_when = Some(condition);
        self
    }

    pub fn default_json(&self) -> Option<serde_json::Value> {
        self.default_value
            .as_ref()
            .map(|default| parameter_value_to_json(&default.value, &self.value_type))
    }
}

pub fn parameter_value_to_json(value: &DataValue, value_type: &TypeExpr) -> serde_json::Value {
    if matches!(value_type, TypeExpr::Concrete(id) if id.as_str() == "core.numeric")
        && let DataValue::Decimal(decimal) = value
        && let Some(number) = decimal
            .as_str()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
    {
        return serde_json::Value::Number(number);
    }
    crate::protocol_value_to_json(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterCondition {
    pub key: ParameterKey,
    pub values: Box<[DataValue]>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterPresentation {
    #[default]
    DetailPanel,
    InlineAndDetail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGroup {
    pub key: ParameterGroupKey,
    pub title_key: I18nKey,
    pub description_key: Option<I18nKey>,
    pub parameters: Box<[Parameter]>,
}

impl ParameterGroup {
    pub fn new(key: &str, parameters: impl IntoIterator<Item = Parameter>) -> Self {
        Self {
            key: ParameterGroupKey::new(key).expect("valid declared parameter group key"),
            title_key: I18nKey::new(format!("parameter_groups.{key}.title"))
                .expect("valid group title"),
            description_key: None,
            parameters: parameters.into_iter().collect(),
        }
    }

    pub fn title(mut self, key: I18nKey) -> Self {
        self.title_key = key;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameters {
    pub groups: Box<[ParameterGroup]>,
}

impl Parameters {
    pub fn new(groups: impl IntoIterator<Item = ParameterGroup>) -> Result<Self, ParametersError> {
        let parameters = Self {
            groups: groups.into_iter().collect(),
        };
        parameters.validate()?;
        Ok(parameters)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Parameter> {
        self.groups.iter().flat_map(|group| group.parameters.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn get(&self, key: &ParameterKey) -> Option<&Parameter> {
        self.iter().find(|parameter| &parameter.key == key)
    }

    pub fn is_visible(&self, parameter: &Parameter, values: &ParameterValues) -> bool {
        parameter.visible_when.as_ref().is_none_or(|condition| {
            let default = (!values.contains_key(&condition.key))
                .then(|| self.get(&condition.key).and_then(Parameter::default_json))
                .flatten();
            values
                .get(&condition.key)
                .or(default.as_ref())
                .is_some_and(|actual| {
                    condition
                        .values
                        .iter()
                        .any(|value| crate::validation::protocol_value_matches_json(value, actual))
                })
        })
    }

    /// Apply a partial edit against current values. Defaults remain protocol-owned;
    /// changing a selector removes inactive values in the same document transaction.
    pub fn merge_values(
        &self,
        current: &ParameterValues,
        changes: ParameterValues,
    ) -> Result<ParameterValues, ParameterKey> {
        let mut merged = current.clone();
        for (key, value) in changes {
            if self.get(&key).is_none() {
                return Err(key);
            }
            if value.is_null() {
                merged.remove(&key);
            } else {
                merged.insert(key, value);
            }
        }
        let inactive: Vec<_> = self
            .iter()
            .filter(|parameter| !self.is_visible(parameter, &merged))
            .map(|parameter| parameter.key.clone())
            .collect();
        for key in inactive {
            merged.remove(&key);
        }
        Ok(merged)
    }

    pub fn validate(&self) -> Result<(), ParametersError> {
        let mut groups = BTreeSet::new();
        let mut parameters = BTreeMap::new();
        for group in &self.groups {
            if !groups.insert(&group.key) {
                return Err(ParametersError::DuplicateGroup(group.key.clone()));
            }
            if group.parameters.is_empty() {
                return Err(ParametersError::EmptyGroup(group.key.clone()));
            }
            for parameter in &group.parameters {
                if parameters.insert(&parameter.key, parameter).is_some() {
                    return Err(ParametersError::DuplicateKey(parameter.key.clone()));
                }
            }
        }
        for parameter in parameters.values() {
            if let Some(value) = parameter.default_json()
                && (!crate::validation::parameter_value_matches_type(&value, &parameter.value_type)
                    || parameter.constraints.iter().any(|constraint| {
                        !crate::validation::parameter_constraint_matches(&value, constraint)
                    }))
            {
                return Err(ParametersError::InvalidDefault(parameter.key.clone()));
            }

            if let Some(condition) = &parameter.visible_when {
                let selector = parameters
                    .get(&condition.key)
                    .filter(|selector| selector.visible_when.is_none())
                    .ok_or_else(|| ParametersError::InvalidCondition(parameter.key.clone()))?;
                if condition.values.is_empty()
                    || condition.values.iter().any(|value| {
                        let value = parameter_value_to_json(value, &selector.value_type);
                        !crate::validation::parameter_value_matches_type(
                            &value,
                            &selector.value_type,
                        ) || selector.constraints.iter().any(|constraint| {
                            !crate::validation::parameter_constraint_matches(&value, constraint)
                        })
                    })
                {
                    return Err(ParametersError::InvalidCondition(parameter.key.clone()));
                }
            }
        }
        Ok(())
    }
}

pub type ParameterValues = BTreeMap<ParameterKey, serde_json::Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterConstraint {
    Required,
    Positive,
    OneOf(Vec<DataValue>),
    IntegerRange { min: Option<i64>, max: Option<i64> },
    Length { min: Option<u32>, max: Option<u32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterEditorSpec {
    Auto,
    Hidden,
    Text { multiline: bool },
    Number,
    Toggle,
    Select,
    GraphConstant,
    SemanticDomain,
    Resource { kind: ResourceDisplayKind },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParametersError {
    DuplicateKey(ParameterKey),
    DuplicateGroup(ParameterGroupKey),
    EmptyGroup(ParameterGroupKey),
    InvalidCondition(ParameterKey),
    InvalidDefault(ParameterKey),
}

impl std::fmt::Display for ParametersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateKey(key) => write!(f, "duplicate parameter key '{key}'"),
            Self::DuplicateGroup(key) => write!(f, "duplicate parameter group '{key}'"),
            Self::EmptyGroup(key) => write!(f, "empty parameter group '{key}'"),
            Self::InvalidDefault(key) => write!(f, "invalid default for parameter '{key}'"),
            Self::InvalidCondition(key) => {
                write!(f, "invalid visibility condition for parameter '{key}'")
            }
        }
    }
}
impl std::error::Error for ParametersError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_declarations_validate_identity_defaults_and_conditions() {
        let mean = Parameter::number("mean").float().default(0.0);
        assert!(matches!(
            Parameters::new([
                ParameterGroup::new("distribution", [mean.clone()]),
                ParameterGroup::new("sampling", [mean.clone()]),
            ]),
            Err(ParametersError::DuplicateKey(_))
        ));
        assert!(matches!(
            Parameters::new([
                ParameterGroup::new("distribution", [mean.clone()]),
                ParameterGroup::new(
                    "distribution",
                    [Parameter::number("count").int().min(1).default(100)]
                ),
            ]),
            Err(ParametersError::DuplicateGroup(_))
        ));
        assert!(matches!(
            Parameters::new([ParameterGroup::new(
                "sampling",
                [Parameter::number("count")
                    .int()
                    .positive()
                    .min(1)
                    .default(0),]
            )]),
            Err(ParametersError::InvalidDefault(_))
        ));
        assert!(matches!(
            Parameters::new([ParameterGroup::new(
                "distribution",
                [mean.when(ParameterCondition {
                    key: "missing".parse().unwrap(),
                    values: Box::new([DataValue::Integer(1)]),
                })]
            )]),
            Err(ParametersError::InvalidCondition(_))
        ));
    }
}
