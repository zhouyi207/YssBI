use serde::{Deserialize, Serialize};

use crate::{
    KnowledgeChunkId, KnowledgeDocumentId, KnowledgeSourceId, PersistenceFailure,
    PersistenceFuture, ProjectSessionBinding, SkillId, SkillVersion, SourceHash, UnixMillis,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillManifest {
    pub id: SkillId,
    pub version: SkillVersion,
    pub source_hash: SourceHash,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillPackage {
    pub manifest: SkillManifest,
    pub instructions: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeSourceStatus {
    Active,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityClass {
    Public,
    Internal,
    Restricted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeSourceRecord {
    pub id: KnowledgeSourceId,
    pub title: String,
    pub version: String,
    pub license: String,
    pub source_hash: SourceHash,
    pub status: KnowledgeSourceStatus,
    pub sensitivity: SensitivityClass,
    pub project: Option<ProjectSessionBinding>,
    pub origin: Option<ProjectKnowledgeOrigin>,
    pub updated_at: UnixMillis,
}

/// Stable project identity and authored resource behind a derived knowledge snapshot.
/// Runtime session bindings are resolved by the Application adapter on every read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectKnowledgeOrigin {
    pub project_key: String,
    pub path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKnowledgeStatus {
    Ready,
    Changed,
    Unavailable,
    Empty,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectKnowledgeSourceSummary {
    pub source_id: KnowledgeSourceId,
    pub title: String,
    pub path: String,
    pub status: ProjectKnowledgeStatus,
    pub updated_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeDocumentRecord {
    pub id: KnowledgeDocumentId,
    pub source_id: KnowledgeSourceId,
    pub title: String,
    pub body: String,
    pub scopes: Vec<String>,
    pub tags: Vec<String>,
    pub source_hash: SourceHash,
    pub project: Option<ProjectSessionBinding>,
    pub sensitivity: SensitivityClass,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeCitation {
    pub source_id: KnowledgeSourceId,
    pub document_id: KnowledgeDocumentId,
    pub chunk_id: KnowledgeChunkId,
    pub title: String,
    pub version: String,
    pub source_hash: SourceHash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeSearchHit {
    pub citation: KnowledgeCitation,
    pub excerpt: String,
    pub score: f32,
}

/// A store-local invalidation token and a coherent view of its active sources.
/// This is internal cache state, never a model or persisted conversation field.
pub struct KnowledgeSourceSnapshot {
    pub generation: u64,
    pub documents: Vec<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>,
}

pub trait KnowledgeSourceStorePort: Send + Sync {
    fn list_active_sources(
        &self,
    ) -> PersistenceFuture<'_, Result<Vec<KnowledgeSourceRecord>, PersistenceFailure>>;

    /// Atomically replace a source and its complete document set. A document ID
    /// already owned by a different source must fail without changing either source.
    fn replace_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
        documents: &'a [KnowledgeDocumentRecord],
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    /// Return None when no knowledge write has occurred since `known_generation`.
    /// Generations are meaningful only for the lifetime of this store instance.
    fn load_active_snapshot<'a>(
        &'a self,
        known_generation: Option<u64>,
    ) -> PersistenceFuture<'a, Result<Option<KnowledgeSourceSnapshot>, PersistenceFailure>>;

    fn read_active_document<'a>(
        &'a self,
        document_id: &'a KnowledgeDocumentId,
    ) -> PersistenceFuture<
        'a,
        Result<Option<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    >;

    fn mark_source_deleted<'a>(
        &'a self,
        source_id: &'a KnowledgeSourceId,
        updated_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;
}
