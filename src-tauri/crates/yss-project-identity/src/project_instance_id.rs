use serde::{Deserialize, Serialize};

/// Stable identity for one activated project instance.
///
/// This is intentionally distinct from [`crate::ProjectSessionId`], which
/// identifies replaceable runtime state derived from an activation.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectInstanceId(String);

impl ProjectInstanceId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_existing(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ProjectInstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProjectInstanceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
