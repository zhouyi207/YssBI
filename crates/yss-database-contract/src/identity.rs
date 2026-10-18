use super::declaration::DatabaseDecl;
use super::fingerprint::fingerprint_declaration;
use serde::{Deserialize, Serialize};

/// Opaque identity of a persisted project database declaration.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DatabaseId(Box<str>);

impl DatabaseId {
    #[must_use]
    pub fn from_existing(value: Box<str>) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

const RESOURCE_PATH_PREFIX: &str = "databases/";

/// Project resource path through which graphs and the project index reference a database.
pub fn database_resource_path(id: &str) -> String {
    format!("{RESOURCE_PATH_PREFIX}{id}")
}

/// The database id named by a project resource path, when the path names a database.
pub fn database_id_in_resource_path(path: &str) -> Option<&str> {
    path.strip_prefix(RESOURCE_PATH_PREFIX)
        .filter(|id| !id.is_empty())
}

/// Monotonic declaration revision observed by a database session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DatabaseDeclarationRevision(u64);

impl DatabaseDeclarationRevision {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Opaque deterministic fingerprint of one complete database declaration.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DatabaseDeclarationFingerprint([u8; 32]);

impl DatabaseDeclarationFingerprint {
    #[must_use]
    pub fn from_decl(declaration: &DatabaseDecl) -> Self {
        Self(fingerprint_declaration(declaration))
    }
}
