use crate::node_system::document::{
    FunctionDocument, GraphDocument, GraphResourcePath, GraphRevision,
};
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
pub type AnalysisResourceReads = ResourceVersionSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "version", rename_all = "snake_case")]
pub enum ResourceObservedState {
    Present(ResourceVersion),
    Absent(Option<ResourceVersion>),
}

pub type ResourceObservationSet = BTreeMap<ResourceKey, ResourceObservedState>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedResource<T> {
    pub key: ResourceKey,
    pub version: ResourceVersion,
    pub value: T,
}

pub struct ResolvedFunctionValue<'a> {
    pub function: &'a FunctionDocument,
    pub graph: &'a GraphDocument,
}

pub type ResolvedFunction<'a> = ResolvedResource<ResolvedFunctionValue<'a>>;
pub type ResolvedVariable<'a> = ResolvedResource<&'a crate::variable::VariableInstance>;
pub type ResolvedDatabase<'a> = ResolvedResource<&'a [crate::schema::ColumnInfoDTO]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceResolutionError {
    key: ResourceKey,
    observed_state: ResourceObservedState,
    reason: Box<str>,
}

impl ResourceResolutionError {
    pub fn new(
        key: ResourceKey,
        observed_state: ResourceObservedState,
        reason: impl Into<Box<str>>,
    ) -> Self {
        Self {
            key,
            observed_state,
            reason: reason.into(),
        }
    }

    pub fn key(&self) -> &ResourceKey {
        &self.key
    }

    pub fn observed_state(&self) -> &ResourceObservedState {
        &self.observed_state
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl std::fmt::Display for ResourceResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for ResourceResolutionError {}

pub trait AnalysisResourceResolver {
    fn resolve_function(
        &mut self,
        path: &GraphResourcePath,
    ) -> Result<ResolvedFunction<'_>, ResourceResolutionError>;

    fn resolve_variable(
        &mut self,
        id: &crate::variable::VariableId,
    ) -> Result<ResolvedVariable<'_>, ResourceResolutionError>;

    fn resolve_database(
        &mut self,
        id: &str,
    ) -> Result<ResolvedDatabase<'_>, ResourceResolutionError>;

    fn reads(&self) -> &AnalysisResourceReads;
    fn observations(&self) -> &ResourceObservationSet;
}

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
    #[serde(default)]
    pub resource_observations: ResourceObservationSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProjection<T> {
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
    pub payload: T,
}
