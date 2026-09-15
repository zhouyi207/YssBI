use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use yss_node_registry::RegistryFingerprint;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "version", rename_all = "snake_case")]
pub enum ResourceObservedState {
    Present(ResourceVersion),
    Absent(Option<ResourceVersion>),
}

pub type ResourceObservationSet = BTreeMap<ResourceKey, ResourceObservedState>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphAnalysisBasis {
    pub registry_fingerprint: RegistryFingerprint,
    pub kernel_fingerprint: [u8; 32],
    pub resource_versions: ResourceVersionSet,
    #[serde(default)]
    pub resource_observations: ResourceObservationSet,
}
