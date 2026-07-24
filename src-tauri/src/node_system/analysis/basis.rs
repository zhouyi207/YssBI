use crate::node_system::document::{GraphResourcePath, GraphRevision};
use crate::node_system::registry::RegistryFingerprint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! string_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<Box<str>>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

string_newtype!(ResourceKey);
string_newtype!(ResourceVersion);

pub type ResourceVersionSet = BTreeMap<ResourceKey, ResourceVersion>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompileId(u64);

impl CompileId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationBasis<GraphRevision> {
    pub graph_revision: GraphRevision,
    pub registry_fingerprint: RegistryFingerprint,
    pub resource_versions: ResourceVersionSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProjection<T> {
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
    pub payload: T,
}
