use serde::{Deserialize, Serialize};

/// Opaque native identity of a project root directory.
///
/// The platform filesystem adapter creates this value. Persistence adapters
/// only preserve and compare it; they must not interpret its contents.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRootIdentity(String);

impl ProjectRootIdentity {
    pub fn from_canonical(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
