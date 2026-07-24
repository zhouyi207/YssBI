use super::{I18nKey, ParameterKey, ParameterValue, TypeExpr, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub key: ParameterKey,
    pub title_key: I18nKey,
    pub description_key: Option<I18nKey>,
    pub value_type: TypeExpr,
    pub default_value: Option<ParameterValue>,
    pub constraints: Vec<ParameterConstraint>,
    pub editor: ParameterEditorSpec,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterSchema {
    pub parameters: Box<[ParameterSpec]>,
}

impl ParameterSchema {
    pub fn new(parameters: Vec<ParameterSpec>) -> Result<Self, DuplicateParameterKey> {
        let mut keys = BTreeSet::new();
        for parameter in &parameters {
            if !keys.insert(parameter.key.clone()) {
                return Err(DuplicateParameterKey(parameter.key.clone()));
            }
        }
        Ok(Self {
            parameters: parameters.into_boxed_slice(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterValues {
    pub values: BTreeMap<ParameterKey, ParameterValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterConstraint {
    Required,
    OneOf(Vec<Value>),
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
    Resource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateParameterKey(pub ParameterKey);

impl std::fmt::Display for DuplicateParameterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "duplicate parameter key '{}'", self.0)
    }
}

impl std::error::Error for DuplicateParameterKey {}
