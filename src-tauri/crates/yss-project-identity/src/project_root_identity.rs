use serde::{Deserialize, Serialize};

/// Opaque native identity of a project root directory.
///
/// Project registration projects a filesystem RootIdentity into this persisted
/// contract. Persistence adapters only preserve and compare the opaque value.
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
