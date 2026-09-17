use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowConcatMode {
    ByName,
    ByPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableJoinKind {
    Inner,
    Left,
    Right,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableJoin {
    pub kind: TableJoinKind,
    pub left_keys: Vec<String>,
    pub right_keys: Vec<String>,
    pub right_suffix: String,
}

impl TableJoin {
    pub fn is_valid(&self) -> bool {
        !self.left_keys.is_empty()
            && self.left_keys.len() == self.right_keys.len()
            && !self.right_suffix.is_empty()
            && [&self.left_keys, &self.right_keys].into_iter().all(|keys| {
                keys.iter().all(|key| !key.is_empty() && key.trim() == key)
                    && keys.iter().collect::<BTreeSet<_>>().len() == keys.len()
            })
    }
}

/// Deterministic names shared by graph schema projection and query construction.
pub fn append_column_names(left: &[String], right: &[String], suffix: &str) -> Vec<String> {
    let mut used = left.iter().cloned().collect::<BTreeSet<_>>();
    right
        .iter()
        .map(|name| {
            let mut candidate = name.clone();
            if used.contains(&candidate) {
                let base = format!("{name}{suffix}");
                candidate = base.clone();
                let mut index = 2;
                while used.contains(&candidate) {
                    candidate = format!("{base}_{index}");
                    index += 1;
                }
            }
            used.insert(candidate.clone());
            candidate
        })
        .collect()
}
