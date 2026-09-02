use serde::{Deserialize, Serialize};

use crate::{
    CapabilityId, HarnessSessionId, KnowledgeChunkId, KnowledgeDocumentId, KnowledgeSourceId,
    MemoryRecordId, PersistenceFailure, PersistenceFuture, ProjectSessionBinding, SkillId,
    SkillVersion, SourceHash, UnixMillis, WorkflowId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    Builtin,
    Project,
    User,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillManifest {
    pub id: SkillId,
    pub version: SkillVersion,
    pub scope: SkillScope,
    pub domain: String,
    pub entry_workflow: WorkflowId,
    pub allowed_capabilities: Vec<CapabilityId>,
    pub knowledge_scopes: Vec<String>,
    pub source_hash: SourceHash,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillPackage {
    pub manifest: SkillManifest,
    pub instructions: String,
}

pub trait SkillSourcePort: Send + Sync {
    fn install_package<'a>(
        &'a self,
        package: &'a SkillPackage,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn list_packages<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<SkillPackage>, PersistenceFailure>>;
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnowledgeSearchHit {
    pub citation: KnowledgeCitation,
    pub excerpt: String,
    pub score: u32,
}

pub trait KnowledgeSourceStorePort: Send + Sync {
    fn upsert_source<'a>(
        &'a self,
        source: &'a KnowledgeSourceRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn upsert_document<'a>(
        &'a self,
        document: &'a KnowledgeDocumentRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn list_active_documents<'a>(
        &'a self,
    ) -> PersistenceFuture<
        'a,
        Result<Vec<(KnowledgeSourceRecord, KnowledgeDocumentRecord)>, PersistenceFailure>,
    >;

    fn mark_source_deleted<'a>(
        &'a self,
        source_id: &'a KnowledgeSourceId,
        updated_at: UnixMillis,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session,
    Project,
    User,
    Episodic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    ResearchQuestion,
    DatasetSemantic,
    VariableRole,
    StudyDesign,
    MethodDecision,
    ModelDecision,
    UserPreference,
    ReportingPreference,
    WorkflowSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum StructuredMemoryValue {
    ResearchQuestion {
        question: String,
    },
    DatasetSemantic {
        resource_id: String,
        meaning: String,
    },
    VariableRole {
        variable: String,
        role: String,
    },
    StudyDesign {
        design: String,
    },
    MethodDecision {
        method_id: String,
        rationale: String,
    },
    ModelDecision {
        model_ref: String,
        rationale: String,
    },
    UserPreference {
        key: String,
        value: String,
    },
    ReportingPreference {
        key: String,
        value: String,
    },
    WorkflowSummary {
        workflow_run_id: String,
        summary: String,
    },
}

impl StructuredMemoryValue {
    pub const fn kind(&self) -> MemoryKind {
        match self {
            Self::ResearchQuestion { .. } => MemoryKind::ResearchQuestion,
            Self::DatasetSemantic { .. } => MemoryKind::DatasetSemantic,
            Self::VariableRole { .. } => MemoryKind::VariableRole,
            Self::StudyDesign { .. } => MemoryKind::StudyDesign,
            Self::MethodDecision { .. } => MemoryKind::MethodDecision,
            Self::ModelDecision { .. } => MemoryKind::ModelDecision,
            Self::UserPreference { .. } => MemoryKind::UserPreference,
            Self::ReportingPreference { .. } => MemoryKind::ReportingPreference,
            Self::WorkflowSummary { .. } => MemoryKind::WorkflowSummary,
        }
    }

    pub fn text_fields(&self) -> Vec<&str> {
        match self {
            Self::ResearchQuestion { question } => vec![question],
            Self::DatasetSemantic {
                resource_id,
                meaning,
            } => vec![resource_id, meaning],
            Self::VariableRole { variable, role } => vec![variable, role],
            Self::StudyDesign { design } => vec![design],
            Self::MethodDecision {
                method_id,
                rationale,
            } => vec![method_id, rationale],
            Self::ModelDecision {
                model_ref,
                rationale,
            } => vec![model_ref, rationale],
            Self::UserPreference { key, value } | Self::ReportingPreference { key, value } => {
                vec![key, value]
            }
            Self::WorkflowSummary {
                workflow_run_id,
                summary,
            } => vec![workflow_run_id, summary],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemorySourceRef {
    pub source_id: String,
    pub source_revision: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryConfidence {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Proposed,
    Approved,
    Active,
    Superseded,
    Invalidated,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAuthor {
    User,
    AgentProposal,
    Workflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    Session,
    Project,
    Persistent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryRecord {
    pub id: MemoryRecordId,
    pub session_id: HarnessSessionId,
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub value: StructuredMemoryValue,
    pub source_refs: Vec<MemorySourceRef>,
    pub confidence: MemoryConfidence,
    pub status: MemoryStatus,
    pub project: Option<ProjectSessionBinding>,
    pub sensitivity: SensitivityClass,
    pub created_by: MemoryAuthor,
    pub supersedes: Option<MemoryRecordId>,
    pub retention: RetentionPolicy,
    pub created_at: UnixMillis,
    pub updated_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryProposal {
    pub session_id: HarnessSessionId,
    pub scope: MemoryScope,
    pub value: StructuredMemoryValue,
    pub source_refs: Vec<MemorySourceRef>,
    pub confidence: MemoryConfidence,
    pub project: Option<ProjectSessionBinding>,
    pub sensitivity: SensitivityClass,
    pub created_by: MemoryAuthor,
    pub supersedes: Option<MemoryRecordId>,
    pub retention: RetentionPolicy,
}

pub trait MemoryStorePort: Send + Sync {
    fn insert<'a>(
        &'a self,
        record: &'a MemoryRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn load<'a>(
        &'a self,
        id: &'a MemoryRecordId,
    ) -> PersistenceFuture<'a, Result<Option<MemoryRecord>, PersistenceFailure>>;

    fn update<'a>(
        &'a self,
        record: &'a MemoryRecord,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn activate<'a>(
        &'a self,
        record: &'a MemoryRecord,
        superseded: Option<&'a MemoryRecord>,
    ) -> PersistenceFuture<'a, Result<(), PersistenceFailure>>;

    fn query_session<'a>(
        &'a self,
        session_id: &'a HarnessSessionId,
    ) -> PersistenceFuture<'a, Result<Vec<MemoryRecord>, PersistenceFailure>>;

    fn list_active<'a>(
        &'a self,
    ) -> PersistenceFuture<'a, Result<Vec<MemoryRecord>, PersistenceFailure>>;
}
