use serde::{Deserialize, Serialize};
use yss_chart_document::ChartEncodings;
use yss_graph_document::{FunctionParameterId, GraphResourcePath};
use yss_project_identity::{OperationId, ResourceRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Graph,
    Function,
    Database,
    Chart,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "key", rename_all = "snake_case")]
pub enum ResourceKey {
    Graph(GraphResourcePath),
    Function(FunctionResourceKey),
    Database(DatabaseResourceKey),
    Chart(ChartResourceKey),
}

impl ResourceKey {
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Graph(_) => ResourceKind::Graph,
            Self::Function(_) => ResourceKind::Function,
            Self::Chart(_) => ResourceKind::Chart,
            Self::Database(_) => ResourceKind::Database,
        }
    }
}

/// Project-owned mutation envelope. Application and Command adapters may
/// carry this envelope across the session boundary, while Graph only receives
/// the typed payload it needs for a single graph operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationRequest<T> {
    pub resource: ResourceKey,
    pub base_revision: ResourceRevision,
    pub operation_id: OperationId,
    pub payload: T,
}

impl<T> MutationRequest<T> {
    pub const fn new(
        resource: ResourceKey,
        base_revision: ResourceRevision,
        operation_id: OperationId,
        payload: T,
    ) -> Self {
        Self {
            resource,
            base_revision,
            operation_id,
            payload,
        }
    }
}

macro_rules! opaque_resource_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Box<str>);
    };
}

opaque_resource_type!(FunctionResourceKey);
opaque_resource_type!(DatabaseResourceKey);
opaque_resource_type!(ChartResourceKey);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub id: FunctionParameterId,
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub parameters: Vec<FunctionParameter>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDocument {
    pub revision: ResourceRevision,
    pub signature: FunctionSignature,
}

impl FunctionDocument {
    pub fn new(signature: FunctionSignature) -> Self {
        Self {
            revision: ResourceRevision::INITIAL,
            signature,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDocumentPatch {
    pub before: FunctionSignature,
    pub after: FunctionSignature,
}

impl FunctionDocumentPatch {
    pub fn new(before: FunctionSignature, after: FunctionSignature) -> Self {
        Self { before, after }
    }

    pub fn inverse(&self) -> Self {
        Self::new(self.after.clone(), self.before.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLifecycleKind {
    Event,
    Function,
    Chart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLifecycleState {
    pub revision: ResourceRevision,
    pub path: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLifecyclePatch {
    pub before: Option<ResourceLifecycleState>,
    pub after: Option<ResourceLifecycleState>,
}

impl ResourceLifecyclePatch {
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcePathMovePatch {
    pub from: Box<str>,
    pub to: Box<str>,
}

impl ResourcePathMovePatch {
    pub fn inverse(&self) -> Self {
        Self {
            from: self.to.clone(),
            to: self.from.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartDocumentState {
    pub database_id: String,
    pub chart_type: String,
    pub encodings: ChartEncodings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartDocumentPatch {
    pub before: ChartDocumentState,
    pub after: ChartDocumentState,
}

impl ChartDocumentPatch {
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseDocumentPatch {
    pub before: Option<yss_database_contract::DatabaseDecl>,
    pub after: Option<yss_database_contract::DatabaseDecl>,
}

impl DatabaseDocumentPatch {
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "patch", rename_all = "snake_case")]
pub enum ResourceDocumentPatch {
    Function(FunctionDocumentPatch),
    Chart(ChartDocumentPatch),
    ResourceLifecycle(ResourceLifecyclePatch),
    ResourceMove(ResourcePathMovePatch),
    Database(DatabaseDocumentPatch),
}

impl ResourceDocumentPatch {
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::ResourceLifecycle(_) | Self::ResourceMove(_) => ResourceKind::Graph,
            Self::Function(_) => ResourceKind::Function,
            Self::Chart(_) => ResourceKind::Chart,
            Self::Database(_) => ResourceKind::Database,
        }
    }

    pub fn inverse(&self) -> Self {
        match self {
            Self::Function(patch) => Self::Function(patch.inverse()),
            Self::Chart(patch) => Self::Chart(patch.inverse()),
            Self::ResourceLifecycle(patch) => Self::ResourceLifecycle(patch.inverse()),
            Self::ResourceMove(patch) => Self::ResourceMove(patch.inverse()),
            Self::Database(patch) => Self::Database(patch.inverse()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDeltaEvent {
    pub resource: ResourceKey,
    pub from_revision: ResourceRevision,
    pub to_revision: ResourceRevision,
    pub caused_by: Option<OperationId>,
    pub payload: ResourceDocumentPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectResourceMutationError {
    #[error("project resource mutation observed a stale project lifecycle: {0}")]
    StaleProjectLifecycle(Box<str>),
    #[error("project resource mutation requires recovery: {0}")]
    RecoveryRequired(Box<str>),
    #[error("project resource mutation conflicted with a resource revision")]
    StaleRevision {
        base_revision: u64,
        current_revision: u64,
    },
    #[error("project resource mutation addressed the wrong resource")]
    ResourceMismatch {
        requested: Box<str>,
        store: Box<str>,
    },
    #[error("project resource mutation could not prepare its projection: {0}")]
    Projection(Box<str>),
    #[error("project resource mutation failed: {0}")]
    Mutation(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGraphResidency {
    Loaded,
    Unloaded,
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn resource_delta_wire_is_camel_case() {
        let caused_by = OperationId::from_uuid(Uuid::from_u128(905));
        let delta = ResourceDeltaEvent {
            resource: ResourceKey::Function(FunctionResourceKey("functions/main".into())),
            from_revision: ResourceRevision::new(4),
            to_revision: ResourceRevision::new(5),
            caused_by: Some(caused_by),
            payload: ResourceDocumentPatch::Function(FunctionDocumentPatch::default()),
        };

        assert_eq!(
            serde_json::to_value(delta).unwrap(),
            json!({
                "resource": {
                    "kind": "function",
                    "key": "functions/main"
                },
                "fromRevision": 4,
                "toRevision": 5,
                "causedBy": "00000000-0000-0000-0000-000000000389",
                "payload": {
                    "kind": "function",
                    "patch": { "before": {"parameters": [], "return_type": null}, "after": {"parameters": [], "return_type": null} }
                }
            })
        );
    }
}
