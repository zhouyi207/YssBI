use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use super::DataType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructTypeMeta {
    pub key: String,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeSystemSnapshot {
    pub struct_types: BTreeMap<String, StructTypeMeta>,
}

impl TypeSystemSnapshot {
    pub fn empty() -> Self {
        Self {
            struct_types: BTreeMap::new(),
        }
    }

    pub fn can_accept(&self, target: &DataType, source: &DataType) -> bool {
        if target == source {
            return true;
        }
        if matches!(target, DataType::Any) || matches!(source, DataType::Any) {
            return true;
        }
        match (source, target) {
            (_, DataType::OneOf(targets)) => targets.iter().any(|t| self.can_accept(t, source)),
            (DataType::OneOf(sources), _) => sources.iter().any(|s| self.can_accept(target, s)),
            (DataType::Array(source_inner), DataType::Array(target_inner)) => {
                self.can_accept(target_inner, source_inner)
            }
            (DataType::DataSeries(source_inner), DataType::DataSeries(target_inner)) => {
                self.can_accept(target_inner, source_inner)
            }
            (DataType::Struct(source_key), DataType::Struct(target_key)) => {
                target_key == source_key || struct_extends(source_key, target_key, self)
            }
            _ => false,
        }
    }
}

fn struct_extends(source_key: &str, target_key: &str, snapshot: &TypeSystemSnapshot) -> bool {
    let mut visited = HashSet::new();
    let mut stack = vec![source_key];

    while let Some(key) = stack.pop() {
        if !visited.insert(key.to_string()) {
            continue;
        }
        let Some(meta) = snapshot.struct_types.get(key) else {
            continue;
        };
        for parent in &meta.parents {
            if parent == target_key {
                return true;
            }
            stack.push(parent);
        }
    }

    false
}
