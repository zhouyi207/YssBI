use serde::{Deserialize, Serialize};

/// Stable identity of one row in the persisted project registry.
///
/// A registration survives project activation and must not be confused with
/// [`crate::ProjectInstanceId`], which identifies one replaceable runtime
/// activation.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRegistrationId(String);

impl ProjectRegistrationId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_existing(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectRegistrationId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}
