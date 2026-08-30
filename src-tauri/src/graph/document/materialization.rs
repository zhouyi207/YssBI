use super::{
    DynamicMemberLocator, DynamicPortBinding, GraphResourcePath, GraphRevision,
    LastKnownPortMetadata, NodeId, OrderKey,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use yss_graph_protocol::{PortDirection, PortKey};

macro_rules! string_token {
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
    };
}

string_token!(CompilationResourceKey);
string_token!(CompilationResourceVersion);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompilationRegistryFingerprint([u8; 32]);

impl CompilationRegistryFingerprint {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub type CompilationResourceVersions = BTreeMap<CompilationResourceKey, CompilationResourceVersion>;

/// Complete authority basis used to resolve a projected graph member.
///
/// This is intentionally document-owned so graph transactions do not depend on
/// compiler or analysis types. An upper layer copies its authoritative basis
/// into this value before issuing a materialization authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationBasisToken {
    graph_path: GraphResourcePath,
    graph_revision: GraphRevision,
    registry_fingerprint: CompilationRegistryFingerprint,
    resource_versions: CompilationResourceVersions,
}

impl CompilationBasisToken {
    pub fn new(
        graph_path: GraphResourcePath,
        graph_revision: GraphRevision,
        registry_fingerprint: CompilationRegistryFingerprint,
        resource_versions: CompilationResourceVersions,
    ) -> Self {
        Self {
            graph_path,
            graph_revision,
            registry_fingerprint,
            resource_versions,
        }
    }

    pub const fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub const fn graph_revision(&self) -> GraphRevision {
        self.graph_revision
    }

    pub const fn registry_fingerprint(&self) -> &CompilationRegistryFingerprint {
        &self.registry_fingerprint
    }

    pub const fn resource_versions(&self) -> &CompilationResourceVersions {
        &self.resource_versions
    }
}

/// Full identity of an unmaterialized member shown by an editor projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedMemberRef {
    basis: CompilationBasisToken,
    node_id: NodeId,
    template: PortKey,
    direction: PortDirection,
    locator: DynamicMemberLocator,
    last_known: LastKnownPortMetadata,
}

impl ProjectedMemberRef {
    pub fn new(
        basis: CompilationBasisToken,
        node_id: NodeId,
        template: PortKey,
        direction: PortDirection,
        locator: DynamicMemberLocator,
        last_known: LastKnownPortMetadata,
    ) -> Self {
        Self {
            basis,
            node_id,
            template,
            direction,
            locator,
            last_known,
        }
    }

    pub const fn basis(&self) -> &CompilationBasisToken {
        &self.basis
    }

    pub const fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub const fn template(&self) -> &PortKey {
        &self.template
    }

    pub const fn direction(&self) -> PortDirection {
        self.direction
    }

    pub const fn locator(&self) -> &DynamicMemberLocator {
        &self.locator
    }

    pub const fn last_known(&self) -> &LastKnownPortMetadata {
        &self.last_known
    }
}

/// Proof that an authoritative resolver validated a projected member.
///
/// Fields are private and construction is crate-restricted. Public consumers
/// can inspect the proof but cannot manufacture one from an IPC payload.
#[derive(Debug, PartialEq, Eq)]
pub struct MaterializationAuthorization {
    member: ProjectedMemberRef,
    order: OrderKey,
}

impl MaterializationAuthorization {
    pub(crate) fn new(member: ProjectedMemberRef, order: OrderKey) -> Self {
        Self { member, order }
    }

    pub const fn member(&self) -> &ProjectedMemberRef {
        &self.member
    }

    pub const fn order(&self) -> &OrderKey {
        &self.order
    }

    pub(crate) fn into_binding(self) -> DynamicPortBinding {
        DynamicPortBinding::Resolved {
            origin: self.member.locator,
            order: self.order,
            last_known: self.member.last_known,
        }
    }
}
