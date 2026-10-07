//! Public topic operations; tree state and validation belong to Project's Mind model.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod edits;
mod reads;
pub use edits::*;
pub use reads::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MindResourceKind {
    Mind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindResourceRef {
    pub kind: MindResourceKind,
    pub id: String,
}

impl MindResourceRef {
    pub fn new(id: String) -> Self {
        Self {
            kind: MindResourceKind::Mind,
            id,
        }
    }
    pub fn resource(&self) -> ProjectResourceRef {
        ProjectResourceRef {
            kind: ProjectResourceKind::Mind,
            id: self.id.clone(),
        }
    }
}

fn topic_id(field: &'static str, id: &str) -> Result<(), CapabilityContractError> {
    if id.is_empty() || id.len() > 128 {
        return Err(CapabilityContractError::InvalidField(field));
    }
    Ok(())
}

pub(crate) fn topic_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), CapabilityContractError> {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        topic_id("topicIds", id)?;
        if !seen.insert(id) {
            return Err(CapabilityContractError::InvalidField("topicIds"));
        }
    }
    Ok(())
}
