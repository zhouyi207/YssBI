use std::collections::{BTreeSet, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use thiserror::Error;

use crate::{MutationPublication, ProjectSession, ProjectState};
use yss_project_filesystem::NormalizedProjectRoot;

use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_project_model::ProjectData;
use yss_variable_contract::{VariableId, VariableInstance};

/// Project-owned identity used when a plan names a resource.
///
/// The value is opaque at the interface. Project only interprets the canonical
/// namespaces it owns while resolving a request against its authoritative data.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectResourceId(Box<str>);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ProjectResourceIdError {
    #[error("resource identity is empty")]
    Empty,
    #[error("resource identity has surrounding whitespace")]
    SurroundingWhitespace,
    #[error("resource identity contains a NUL")]
    Nul,
}

impl ProjectResourceId {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, ProjectResourceIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProjectResourceIdError::Empty);
        }
        if value.trim() != value.as_ref() {
            return Err(ProjectResourceIdError::SurroundingWhitespace);
        }
        if value.contains('\0') {
            return Err(ProjectResourceIdError::Nul);
        }
        Ok(Self(value))
    }

    pub fn variable(id: VariableId) -> Self {
        Self::from_existing(format!("variables/{id}").into_boxed_str())
    }

    pub fn database(id: impl AsRef<str>) -> Result<Self, ProjectResourceIdError> {
        Self::new(format!("databases/{}", id.as_ref()).into_boxed_str())
    }

    pub fn graph(path: &GraphResourcePath) -> Self {
        Self::from_existing(path.as_str().into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_existing(value: Box<str>) -> Self {
        debug_assert!(Self::new(value.clone()).is_ok());
        Self(value)
    }
}

impl std::fmt::Display for ProjectResourceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectResourceKind {
    DatabaseConnection,
    DataFrame,
    File,
    Variable,
    Plot,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectResourceAccess {
    Shared,
    Exclusive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectResourcePresence {
    Present,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectResourceVersion(u64);

impl ProjectResourceVersion {
    pub const INITIAL: Self = Self(0);

    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn from_revision(revision: ResourceRevision) -> Self {
        Self(revision.get())
    }

    pub const fn from_graph_revision(revision: GraphRevision) -> Self {
        Self(revision.get())
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectResourceRequirement {
    resource: ProjectResourceId,
    kind: ProjectResourceKind,
    access: ProjectResourceAccess,
    optional: bool,
}

impl ProjectResourceRequirement {
    pub fn new(
        resource: ProjectResourceId,
        kind: ProjectResourceKind,
        access: ProjectResourceAccess,
        optional: bool,
    ) -> Self {
        Self {
            resource,
            kind,
            access,
            optional,
        }
    }

    pub fn resource(&self) -> &ProjectResourceId {
        &self.resource
    }

    pub const fn kind(&self) -> ProjectResourceKind {
        self.kind
    }

    pub const fn access(&self) -> ProjectResourceAccess {
        self.access
    }

    pub const fn optional(&self) -> bool {
        self.optional
    }
}

/// Exact Project-side evidence captured for one required resource.
///
/// A candidate must repeat every field before Project will mint a prepared
/// commit. The type deliberately contains no runtime/database handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectResourceGrant {
    resource: ProjectResourceId,
    kind: ProjectResourceKind,
    access: ProjectResourceAccess,
    optional: bool,
    presence: ProjectResourcePresence,
    version: Option<ProjectResourceVersion>,
}

impl ProjectResourceGrant {
    pub fn new(
        resource: ProjectResourceId,
        kind: ProjectResourceKind,
        access: ProjectResourceAccess,
        optional: bool,
        presence: ProjectResourcePresence,
        version: Option<ProjectResourceVersion>,
    ) -> Self {
        Self {
            resource,
            kind,
            access,
            optional,
            presence,
            version,
        }
    }

    pub fn resource(&self) -> &ProjectResourceId {
        &self.resource
    }

    pub const fn kind(&self) -> ProjectResourceKind {
        self.kind
    }

    pub const fn access(&self) -> ProjectResourceAccess {
        self.access
    }

    pub const fn optional(&self) -> bool {
        self.optional
    }

    pub const fn presence(&self) -> ProjectResourcePresence {
        self.presence
    }

    pub const fn version(&self) -> Option<ProjectResourceVersion> {
        self.version
    }
}

#[derive(Clone, Debug)]
pub struct ProjectExecutionRequest {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub required_resources: Box<[ProjectResourceRequirement]>,
}

impl ProjectExecutionRequest {
    pub fn new(project_instance_id: ProjectInstanceId, graph_path: GraphResourcePath) -> Self {
        Self {
            project_instance_id,
            graph_path,
            required_resources: Box::new([]),
        }
    }

    pub fn with_required_resources(
        mut self,
        requirements: impl IntoIterator<Item = ProjectResourceRequirement>,
    ) -> Self {
        self.required_resources = requirements
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self
    }

    pub fn required_resources(&self) -> &[ProjectResourceRequirement] {
        &self.required_resources
    }
}

#[derive(Clone, Debug)]
pub struct ProjectExecutionResourceSnapshot {
    grants: Arc<[ProjectResourceGrant]>,
}

impl ProjectExecutionResourceSnapshot {
    pub fn grants(&self) -> &[ProjectResourceGrant] {
        &self.grants
    }
}

#[derive(Clone, Debug)]
pub struct ProjectExecutionAuthority {
    session: ProjectSession,
    graph_path: GraphResourcePath,
    graph_revision: GraphRevision,
    document: Arc<GraphDocument>,
    authority_generation: u64,
    resource_grants: Arc<[ProjectResourceGrant]>,
}

impl ProjectExecutionAuthority {
    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.session.instance_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub fn graph_revision(&self) -> GraphRevision {
        self.graph_revision
    }

    pub fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub const fn authority_generation(&self) -> u64 {
        self.authority_generation
    }

    pub fn required_resource_grant_basis(&self) -> &[ProjectResourceGrant] {
        &self.resource_grants
    }
}

pub struct PreparedProjectExecution {
    authority: ProjectExecutionAuthority,
    graph: Arc<GraphDocument>,
    resources: ProjectExecutionResourceSnapshot,
}

impl PreparedProjectExecution {
    pub fn authority(&self) -> &ProjectExecutionAuthority {
        &self.authority
    }

    pub fn graph(&self) -> &GraphDocument {
        &self.graph
    }

    pub fn resources(&self) -> &ProjectExecutionResourceSnapshot {
        &self.resources
    }
}

#[derive(Debug)]
pub struct CandidateVariableWrite {
    grant: ProjectResourceGrant,
    value: VariableInstance,
}

impl CandidateVariableWrite {
    pub fn new(grant: ProjectResourceGrant, value: VariableInstance) -> Self {
        Self { grant, value }
    }

    pub fn grant(&self) -> &ProjectResourceGrant {
        &self.grant
    }

    pub fn value(&self) -> &VariableInstance {
        &self.value
    }
}

#[derive(Debug)]
pub struct CandidateProjectEffects {
    grants: Box<[ProjectResourceGrant]>,
    variable_writes: Box<[CandidateVariableWrite]>,
}

impl CandidateProjectEffects {
    pub fn new(
        grants: impl IntoIterator<Item = ProjectResourceGrant>,
        variable_writes: impl IntoIterator<Item = CandidateVariableWrite>,
    ) -> Self {
        Self {
            grants: grants.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            variable_writes: variable_writes
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    pub fn empty() -> Self {
        Self::new([], [])
    }
}

pub struct PreparedEffectCommit {
    authority: ProjectExecutionAuthority,
    effects: CandidateProjectEffects,
}

impl PreparedEffectCommit {
    pub fn authority(&self) -> &ProjectExecutionAuthority {
        &self.authority
    }
}

pub struct CommittedProjectEffects {
    project_instance_id: ProjectInstanceId,
    authority_generation: u64,
    publication_revision: u64,
    resource_grants: Box<[ProjectResourceGrant]>,
    variable_ids: Box<[VariableId]>,
}

impl CommittedProjectEffects {
    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }

    pub const fn authority_generation(&self) -> u64 {
        self.authority_generation
    }

    pub const fn publication_revision(&self) -> u64 {
        self.publication_revision
    }

    pub fn resource_grants(&self) -> &[ProjectResourceGrant] {
        &self.resource_grants
    }

    pub fn variable_ids(&self) -> &[VariableId] {
        &self.variable_ids
    }
}

#[derive(Clone)]
pub struct ProjectEffectCommitControl {
    cancellation: Arc<AtomicBool>,
    deadline: Instant,
}

impl ProjectEffectCommitControl {
    pub fn new(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation,
            deadline,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ProjectExecutionPreparationError {
    #[error("project execution authority is unavailable")]
    Unavailable,
    #[error("project execution request belongs to another project")]
    ProjectIdentityMismatch {
        requested: ProjectInstanceId,
        current: ProjectInstanceId,
    },
    #[error("requested graph is unavailable: {graph}")]
    GraphUnavailable { graph: GraphResourcePath },
    #[error("requested graph has no matching revision authority: {graph}")]
    GraphRevisionUnavailable { graph: GraphResourcePath },
    #[error("requested graph is invalid: {graph}")]
    InvalidGraph { graph: GraphResourcePath },
    #[error("resource requirement is duplicated: {resource}")]
    DuplicateResourceRequirement { resource: ProjectResourceId },
    #[error("resource identity cannot be resolved: {resource}")]
    InvalidResourceIdentity { resource: ProjectResourceId },
    #[error("resource is unavailable: {resource}")]
    ResourceUnavailable { resource: ProjectResourceId },
    #[error("resource has no revision authority: {resource}")]
    ResourceRevisionUnavailable { resource: ProjectResourceId },
    #[error("resource kind does not match its Project-owned namespace: {resource}")]
    ResourceKindMismatch {
        resource: ProjectResourceId,
        requested: ProjectResourceKind,
        actual: ProjectResourceKind,
    },
    #[error("resource kind is not owned by Project: {resource}")]
    UnsupportedResourceKind {
        resource: ProjectResourceId,
        kind: ProjectResourceKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ProjectEffectCommitError {
    #[error("project effect commit authority is unavailable")]
    ProjectUnavailable,
    #[error("project effect commit authority is stale")]
    StaleProjectSession,
    #[error("project graph document changed before effect commit")]
    GraphChanged,
    #[error("project graph revision changed before effect commit")]
    GraphRevisionChanged {
        expected: GraphRevision,
        current: GraphRevision,
    },
    #[error("project graph revision authority changed before effect commit")]
    GraphRevisionAuthorityChanged,
    #[error("project effect dependency is unavailable")]
    ResourceUnavailable { resource: ProjectResourceId },
    #[error("project effect dependency has no revision authority")]
    ResourceRevisionUnavailable { resource: ProjectResourceId },
    #[error("project effect dependency kind changed")]
    ResourceKindChanged { resource: ProjectResourceId },
    #[error("project effect dependency presence changed")]
    ResourcePresenceChanged { resource: ProjectResourceId },
    #[error("project effect dependency version changed")]
    ResourceVersionChanged { resource: ProjectResourceId },
    #[error("candidate grant is duplicated")]
    DuplicateCandidateGrant { resource: ProjectResourceId },
    #[error("candidate grant is missing")]
    MissingCandidateGrant { resource: ProjectResourceId },
    #[error("candidate grant is unexpected")]
    UnexpectedCandidateGrant { resource: ProjectResourceId },
    #[error("candidate grant kind does not match")]
    CandidateKindMismatch { resource: ProjectResourceId },
    #[error("candidate grant access does not match")]
    CandidateAccessMismatch { resource: ProjectResourceId },
    #[error("candidate grant optionality does not match")]
    CandidateOptionalityMismatch { resource: ProjectResourceId },
    #[error("candidate grant presence does not match")]
    CandidatePresenceMismatch { resource: ProjectResourceId },
    #[error("candidate grant version does not match")]
    CandidateVersionMismatch { resource: ProjectResourceId },
    #[error("candidate variable effect is duplicated")]
    DuplicateCandidateEffect { resource: ProjectResourceId },
    #[error("candidate variable effect has no matching grant")]
    CandidateEffectWithoutGrant { resource: ProjectResourceId },
    #[error("candidate variable effect is invalid")]
    InvalidVariableEffect { resource: ProjectResourceId },
    #[error("variable effect revision is exhausted")]
    VariableRevisionExhausted { resource: ProjectResourceId },
    #[error("project effect commit was cancelled")]
    Cancelled,
    #[error("project effect commit deadline was exceeded")]
    DeadlineExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceFamilyKind {
    Database,
    File,
    Variable,
}

enum ResourceIdentity {
    Database(Box<str>),
    File(GraphResourcePath),
    Variable(VariableId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceResolutionFailure {
    InvalidIdentity,
    UnknownResource,
    UnsupportedKind,
    KindMismatch { actual: ProjectResourceKind },
    Unavailable,
    RevisionUnavailable,
}

fn identify_resource(
    resource: &ProjectResourceId,
) -> Result<ResourceIdentity, ResourceResolutionFailure> {
    let raw = resource.as_str();
    if let Some(value) = raw.strip_prefix("variables/") {
        let id = uuid::Uuid::parse_str(value)
            .map(VariableId::from)
            .map_err(|_| ResourceResolutionFailure::InvalidIdentity)?;
        return Ok(ResourceIdentity::Variable(id));
    }
    if let Some(value) = raw.strip_prefix("databases/") {
        if value.is_empty() {
            return Err(ResourceResolutionFailure::InvalidIdentity);
        }
        return Ok(ResourceIdentity::Database(value.into()));
    }
    if raw.starts_with("events/") || raw.starts_with("functions/") {
        let path =
            GraphResourcePath::new(raw).map_err(|_| ResourceResolutionFailure::InvalidIdentity)?;
        return Ok(ResourceIdentity::File(path));
    }
    Err(ResourceResolutionFailure::UnknownResource)
}

fn resource_family_kind(identity: &ResourceIdentity) -> ResourceFamilyKind {
    match identity {
        ResourceIdentity::Database(_) => ResourceFamilyKind::Database,
        ResourceIdentity::File(_) => ResourceFamilyKind::File,
        ResourceIdentity::Variable(_) => ResourceFamilyKind::Variable,
    }
}

fn validate_requested_kind(
    identity: &ResourceIdentity,
    requested: ProjectResourceKind,
) -> Result<(), ResourceResolutionFailure> {
    if requested == ProjectResourceKind::Plot {
        return Err(ResourceResolutionFailure::UnsupportedKind);
    }
    let family = resource_family_kind(identity);
    let valid = match family {
        ResourceFamilyKind::Database => {
            matches!(
                requested,
                ProjectResourceKind::DatabaseConnection | ProjectResourceKind::DataFrame
            )
        }
        ResourceFamilyKind::File => requested == ProjectResourceKind::File,
        ResourceFamilyKind::Variable => requested == ProjectResourceKind::Variable,
    };
    if valid {
        return Ok(());
    }
    let actual = match family {
        ResourceFamilyKind::Database => ProjectResourceKind::DatabaseConnection,
        ResourceFamilyKind::File => ProjectResourceKind::File,
        ResourceFamilyKind::Variable => ProjectResourceKind::Variable,
    };
    Err(ResourceResolutionFailure::KindMismatch { actual })
}

fn resource_grant_from_requirement(
    requirement: &ProjectResourceRequirement,
    data: &ProjectData,
    graph_revisions: &HashMap<GraphResourcePath, GraphRevision>,
    variable_revisions: &HashMap<VariableId, crate::project_state::VariableRevisionEntry>,
    database_revisions: &HashMap<String, u64>,
) -> Result<ProjectResourceGrant, ResourceResolutionFailure> {
    let identity = identify_resource(&requirement.resource)?;
    validate_requested_kind(&identity, requirement.kind)?;
    let (presence, version) = match identity {
        ResourceIdentity::Database(id) => {
            let version = database_revisions
                .get(id.as_ref())
                .copied()
                .map(ProjectResourceVersion::from_existing);
            if data.databases.contains_key(id.as_ref()) {
                let Some(version) = version else {
                    return Err(ResourceResolutionFailure::RevisionUnavailable);
                };
                (ProjectResourcePresence::Present, Some(version))
            } else {
                if !requirement.optional {
                    return Err(ResourceResolutionFailure::Unavailable);
                }
                (ProjectResourcePresence::Absent, version)
            }
        }
        ResourceIdentity::File(path) => {
            let version = graph_revisions
                .get(&path)
                .copied()
                .map(ProjectResourceVersion::from_graph_revision);
            if data.graphs.contains_key(&path) {
                let Some(version) = version else {
                    return Err(ResourceResolutionFailure::RevisionUnavailable);
                };
                (ProjectResourcePresence::Present, Some(version))
            } else {
                if !requirement.optional {
                    return Err(ResourceResolutionFailure::Unavailable);
                }
                (ProjectResourcePresence::Absent, version)
            }
        }
        ResourceIdentity::Variable(id) => {
            let Some(entry) = variable_revisions.get(&id).copied() else {
                return if data.variables.contains_key(&id) {
                    Err(ResourceResolutionFailure::RevisionUnavailable)
                } else if requirement.optional {
                    Ok(ProjectResourceGrant::new(
                        requirement.resource.clone(),
                        requirement.kind,
                        requirement.access,
                        requirement.optional,
                        ProjectResourcePresence::Absent,
                        None,
                    ))
                } else {
                    Err(ResourceResolutionFailure::Unavailable)
                };
            };
            if data.variables.contains_key(&id) {
                if !entry.is_present() {
                    return Err(ResourceResolutionFailure::RevisionUnavailable);
                }
                (
                    ProjectResourcePresence::Present,
                    Some(ProjectResourceVersion::from_revision(entry.revision)),
                )
            } else {
                if entry.is_present() {
                    return Err(ResourceResolutionFailure::RevisionUnavailable);
                }
                if !requirement.optional {
                    return Err(ResourceResolutionFailure::Unavailable);
                }
                (
                    ProjectResourcePresence::Absent,
                    Some(ProjectResourceVersion::from_revision(entry.revision)),
                )
            }
        }
    };
    Ok(ProjectResourceGrant::new(
        requirement.resource.clone(),
        requirement.kind,
        requirement.access,
        requirement.optional,
        presence,
        version,
    ))
}

fn preparation_resolution_error(
    resource: &ProjectResourceId,
    failure: ResourceResolutionFailure,
    requirement: &ProjectResourceRequirement,
) -> ProjectExecutionPreparationError {
    match failure {
        ResourceResolutionFailure::KindMismatch { actual } => {
            ProjectExecutionPreparationError::ResourceKindMismatch {
                resource: resource.clone(),
                requested: requirement.kind,
                actual,
            }
        }
        ResourceResolutionFailure::UnsupportedKind => {
            ProjectExecutionPreparationError::UnsupportedResourceKind {
                resource: resource.clone(),
                kind: requirement.kind,
            }
        }
        ResourceResolutionFailure::RevisionUnavailable => {
            ProjectExecutionPreparationError::ResourceRevisionUnavailable {
                resource: resource.clone(),
            }
        }
        ResourceResolutionFailure::Unavailable => {
            ProjectExecutionPreparationError::ResourceUnavailable {
                resource: resource.clone(),
            }
        }
        ResourceResolutionFailure::InvalidIdentity | ResourceResolutionFailure::UnknownResource => {
            ProjectExecutionPreparationError::InvalidResourceIdentity {
                resource: resource.clone(),
            }
        }
    }
}

fn current_resolution_error(
    resource: &ProjectResourceId,
    failure: ResourceResolutionFailure,
) -> ProjectEffectCommitError {
    match failure {
        ResourceResolutionFailure::RevisionUnavailable => {
            ProjectEffectCommitError::ResourceRevisionUnavailable {
                resource: resource.clone(),
            }
        }
        ResourceResolutionFailure::Unavailable
        | ResourceResolutionFailure::InvalidIdentity
        | ResourceResolutionFailure::UnknownResource
        | ResourceResolutionFailure::UnsupportedKind => {
            ProjectEffectCommitError::ResourceUnavailable {
                resource: resource.clone(),
            }
        }
        ResourceResolutionFailure::KindMismatch { .. } => {
            ProjectEffectCommitError::ResourceKindChanged {
                resource: resource.clone(),
            }
        }
    }
}

fn current_project_session(
    publication: &MutationPublication,
    project_path: &Option<String>,
) -> Result<ProjectSession, ()> {
    let path = project_path.as_deref().ok_or(())?;
    let root = NormalizedProjectRoot::from_project_path(path).map_err(|_| ())?;
    Ok(ProjectSession {
        instance_id: ProjectInstanceId::from_existing(publication.project_instance_id.clone()),
        root,
    })
}

fn compare_candidate_grant(
    expected: &ProjectResourceGrant,
    actual: &ProjectResourceGrant,
) -> Result<(), ProjectEffectCommitError> {
    if expected.kind != actual.kind {
        return Err(ProjectEffectCommitError::CandidateKindMismatch {
            resource: actual.resource.clone(),
        });
    }
    if expected.access != actual.access {
        return Err(ProjectEffectCommitError::CandidateAccessMismatch {
            resource: actual.resource.clone(),
        });
    }
    if expected.optional != actual.optional {
        return Err(ProjectEffectCommitError::CandidateOptionalityMismatch {
            resource: actual.resource.clone(),
        });
    }
    if expected.presence != actual.presence {
        return Err(ProjectEffectCommitError::CandidatePresenceMismatch {
            resource: actual.resource.clone(),
        });
    }
    if expected.version != actual.version {
        return Err(ProjectEffectCommitError::CandidateVersionMismatch {
            resource: actual.resource.clone(),
        });
    }
    Ok(())
}

fn compare_current_grant(
    expected: &ProjectResourceGrant,
    current: &ProjectResourceGrant,
) -> Result<(), ProjectEffectCommitError> {
    if expected.kind != current.kind {
        return Err(ProjectEffectCommitError::ResourceKindChanged {
            resource: expected.resource.clone(),
        });
    }
    if expected.presence != current.presence {
        return Err(ProjectEffectCommitError::ResourcePresenceChanged {
            resource: expected.resource.clone(),
        });
    }
    if expected.version != current.version {
        return Err(ProjectEffectCommitError::ResourceVersionChanged {
            resource: expected.resource.clone(),
        });
    }
    Ok(())
}

fn validate_candidate_effects(
    authority: &ProjectExecutionAuthority,
    effects: &CandidateProjectEffects,
) -> Result<(), ProjectEffectCommitError> {
    let mut seen = BTreeSet::new();
    for candidate in &effects.grants {
        let resource = candidate.resource.clone();
        if !seen.insert(resource.clone()) {
            return Err(ProjectEffectCommitError::DuplicateCandidateGrant { resource });
        }
        let Some(expected) = authority
            .resource_grants
            .iter()
            .find(|grant| grant.resource == candidate.resource)
        else {
            return Err(ProjectEffectCommitError::UnexpectedCandidateGrant { resource });
        };
        compare_candidate_grant(expected, candidate)?;
    }
    for expected in authority.resource_grants.iter() {
        if !seen.contains(&expected.resource) {
            return Err(ProjectEffectCommitError::MissingCandidateGrant {
                resource: expected.resource.clone(),
            });
        }
    }

    let mut effect_resources = BTreeSet::new();
    for effect in &effects.variable_writes {
        let resource = effect.grant.resource.clone();
        if !effect_resources.insert(resource.clone()) {
            return Err(ProjectEffectCommitError::DuplicateCandidateEffect { resource });
        }
        let Some(grant) = effects
            .grants
            .iter()
            .find(|grant| grant.resource == effect.grant.resource)
        else {
            return Err(ProjectEffectCommitError::CandidateEffectWithoutGrant { resource });
        };
        compare_candidate_grant(grant, &effect.grant)?;
        if effect.grant.kind != ProjectResourceKind::Variable
            || effect.grant.access != ProjectResourceAccess::Exclusive
            || effect.grant.presence != ProjectResourcePresence::Present
            || effect.grant.version.is_none()
        {
            return Err(ProjectEffectCommitError::InvalidVariableEffect { resource });
        }
        let Ok(ResourceIdentity::Variable(variable_id)) = identify_resource(&effect.grant.resource)
        else {
            return Err(ProjectEffectCommitError::InvalidVariableEffect { resource });
        };
        if effect.value.id != variable_id {
            return Err(ProjectEffectCommitError::InvalidVariableEffect { resource });
        }
    }
    Ok(())
}

struct CurrentAuthorityContents<'a> {
    publication: &'a MutationPublication,
    project_path: &'a Option<String>,
    identity: &'a crate::project_state::ProjectAuthorityExpectation,
    data: &'a ProjectData,
    graph_revisions: &'a HashMap<GraphResourcePath, GraphRevision>,
    variable_revisions: &'a HashMap<VariableId, crate::project_state::VariableRevisionEntry>,
    database_revisions: &'a HashMap<String, u64>,
}

fn validate_current_authority_contents(
    authority: &ProjectExecutionAuthority,
    current: CurrentAuthorityContents<'_>,
) -> Result<(), ProjectEffectCommitError> {
    let CurrentAuthorityContents {
        publication,
        project_path,
        identity,
        data,
        graph_revisions,
        variable_revisions,
        database_revisions,
    } = current;
    let session = current_project_session(publication, project_path)
        .map_err(|_| ProjectEffectCommitError::StaleProjectSession)?;
    if session != authority.session
        || identity.project_instance_id != authority.session.instance_id
        || identity.project_root.as_ref() != Some(&authority.session.root)
    {
        return Err(ProjectEffectCommitError::StaleProjectSession);
    }
    if publication.project_instance_id != authority.session.instance_id.as_str() {
        return Err(ProjectEffectCommitError::StaleProjectSession);
    }
    let graph = data
        .graphs
        .get(&authority.graph_path)
        .ok_or(ProjectEffectCommitError::StaleProjectSession)?;
    if graph.document.revision != authority.graph_revision {
        return Err(ProjectEffectCommitError::GraphRevisionChanged {
            expected: authority.graph_revision,
            current: graph.document.revision,
        });
    }
    if graph.document != *authority.document {
        return Err(ProjectEffectCommitError::GraphChanged);
    }
    if graph_revisions.get(&authority.graph_path) != Some(&authority.graph_revision) {
        return Err(ProjectEffectCommitError::GraphRevisionAuthorityChanged);
    }
    for expected in authority.resource_grants.iter() {
        let requirement = ProjectResourceRequirement::new(
            expected.resource.clone(),
            expected.kind,
            expected.access,
            expected.optional,
        );
        let current = resource_grant_from_requirement(
            &requirement,
            data,
            graph_revisions,
            variable_revisions,
            database_revisions,
        )
        .map_err(|failure| current_resolution_error(&expected.resource, failure))?;
        compare_current_grant(expected, &current)?;
    }
    Ok(())
}

fn validate_variable_effect_state(
    effects: &CandidateProjectEffects,
    data: &ProjectData,
    variable_revisions: &HashMap<VariableId, crate::project_state::VariableRevisionEntry>,
) -> Result<(), ProjectEffectCommitError> {
    for effect in &effects.variable_writes {
        let resource = effect.grant.resource.clone();
        let Ok(ResourceIdentity::Variable(variable_id)) = identify_resource(&resource) else {
            return Err(ProjectEffectCommitError::InvalidVariableEffect { resource });
        };
        let Some(current) = data.variables.get(&variable_id) else {
            return Err(ProjectEffectCommitError::ResourceUnavailable { resource });
        };
        if current.id != effect.value.id {
            return Err(ProjectEffectCommitError::InvalidVariableEffect { resource });
        }
        let Some(entry) = variable_revisions.get(&variable_id).copied() else {
            return Err(ProjectEffectCommitError::ResourceRevisionUnavailable { resource });
        };
        if !entry.is_present()
            || Some(ProjectResourceVersion::from_revision(entry.revision)) != effect.grant.version
        {
            return Err(ProjectEffectCommitError::ResourceVersionChanged { resource });
        }
    }
    Ok(())
}

impl ProjectState {
    pub fn prepare_execution(
        &self,
        request: ProjectExecutionRequest,
    ) -> Result<PreparedProjectExecution, ProjectExecutionPreparationError> {
        self.ensure_project_operational()
            .map_err(|_| ProjectExecutionPreparationError::Unavailable)?;
        let lifecycle_before = self.activation_generation.load(Ordering::Acquire);
        if !lifecycle_before.is_multiple_of(2) {
            return Err(ProjectExecutionPreparationError::Unavailable);
        }

        let publication = self
            .mutation_publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let project_path = self
            .project_path
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let session = current_project_session(&publication, &project_path)
            .map_err(|_| ProjectExecutionPreparationError::Unavailable)?;
        if session.instance_id != request.project_instance_id {
            return Err(ProjectExecutionPreparationError::ProjectIdentityMismatch {
                requested: request.project_instance_id,
                current: session.instance_id,
            });
        }
        let identity = self
            .activation_identity
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if identity.project_instance_id != session.instance_id
            || identity.project_root.as_ref() != Some(&session.root)
        {
            return Err(ProjectExecutionPreparationError::Unavailable);
        }
        let data = self
            .project_data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let graph = data.graphs.get(&request.graph_path).ok_or_else(|| {
            ProjectExecutionPreparationError::GraphUnavailable {
                graph: request.graph_path.clone(),
            }
        })?;
        let graph_revision = graph.document.revision;
        let graph_revisions = self
            .graph_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if graph_revisions.get(&request.graph_path) != Some(&graph_revision) {
            return Err(ProjectExecutionPreparationError::GraphRevisionUnavailable {
                graph: request.graph_path.clone(),
            });
        }
        let variable_revisions = self
            .variable_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let database_revisions = self
            .database_authority_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut seen = BTreeSet::new();
        let grants = request
            .required_resources
            .iter()
            .map(|requirement| {
                if !seen.insert(requirement.resource.clone()) {
                    return Err(
                        ProjectExecutionPreparationError::DuplicateResourceRequirement {
                            resource: requirement.resource.clone(),
                        },
                    );
                }
                resource_grant_from_requirement(
                    requirement,
                    &data,
                    &graph_revisions,
                    &variable_revisions,
                    &database_revisions,
                )
                .map_err(|failure| {
                    preparation_resolution_error(&requirement.resource, failure, requirement)
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let resource_grants: Arc<[ProjectResourceGrant]> = Arc::from(grants);
        let document = Arc::new(graph.document.clone());
        let authority = ProjectExecutionAuthority {
            session,
            graph_path: request.graph_path,
            graph_revision,
            document: Arc::clone(&document),
            authority_generation: publication.authority_generation(),
            resource_grants: Arc::clone(&resource_grants),
        };
        let lifecycle_after = self.activation_generation.load(Ordering::Acquire);
        if lifecycle_before != lifecycle_after || !lifecycle_after.is_multiple_of(2) {
            return Err(ProjectExecutionPreparationError::Unavailable);
        }
        Ok(PreparedProjectExecution {
            authority,
            graph: document,
            resources: ProjectExecutionResourceSnapshot {
                grants: resource_grants,
            },
        })
    }

    pub fn prepare_execution_effects(
        &self,
        authority: &ProjectExecutionAuthority,
        effects: CandidateProjectEffects,
    ) -> Result<PreparedEffectCommit, ProjectEffectCommitError> {
        self.ensure_project_operational()
            .map_err(|_| ProjectEffectCommitError::ProjectUnavailable)?;
        let lifecycle_before = self.activation_generation.load(Ordering::Acquire);
        if !lifecycle_before.is_multiple_of(2) {
            return Err(ProjectEffectCommitError::StaleProjectSession);
        }
        let publication = self
            .mutation_publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let project_path = self
            .project_path
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity = self
            .activation_identity
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let data = self
            .project_data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let graph_revisions = self
            .graph_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let variable_revisions = self
            .variable_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let database_revisions = self
            .database_authority_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        validate_current_authority_contents(
            authority,
            CurrentAuthorityContents {
                publication: &publication,
                project_path: &project_path,
                identity: &identity,
                data: &data,
                graph_revisions: &graph_revisions,
                variable_revisions: &variable_revisions,
                database_revisions: &database_revisions,
            },
        )?;
        validate_candidate_effects(authority, &effects)?;
        validate_variable_effect_state(&effects, &data, &variable_revisions)?;
        let lifecycle_after = self.activation_generation.load(Ordering::Acquire);
        if lifecycle_before != lifecycle_after || !lifecycle_after.is_multiple_of(2) {
            return Err(ProjectEffectCommitError::StaleProjectSession);
        }
        Ok(PreparedEffectCommit {
            authority: authority.clone(),
            effects,
        })
    }

    pub fn finalize_execution_effects(
        &self,
        prepared: PreparedEffectCommit,
        control: &ProjectEffectCommitControl,
    ) -> Result<CommittedProjectEffects, ProjectEffectCommitError> {
        check_commit_control(control)?;
        self.ensure_project_operational()
            .map_err(|_| ProjectEffectCommitError::ProjectUnavailable)?;
        let lifecycle_before = self.activation_generation.load(Ordering::Acquire);
        if !lifecycle_before.is_multiple_of(2) {
            return Err(ProjectEffectCommitError::StaleProjectSession);
        }

        let PreparedEffectCommit { authority, effects } = prepared;
        let mut publication = self
            .mutation_publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let project_path = self
            .project_path
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity = self
            .activation_identity
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut data = self
            .project_data
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let graph_revisions = self
            .graph_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut variable_revisions = self
            .variable_revisions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let database_revisions = self
            .database_authority_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        check_commit_control(control)?;
        validate_current_authority_contents(
            &authority,
            CurrentAuthorityContents {
                publication: &publication,
                project_path: &project_path,
                identity: &identity,
                data: &data,
                graph_revisions: &graph_revisions,
                variable_revisions: &variable_revisions,
                database_revisions: &database_revisions,
            },
        )?;
        validate_candidate_effects(&authority, &effects)?;
        validate_variable_effect_state(&effects, &data, &variable_revisions)?;

        let mut next_data = data.clone();
        let mut next_variable_revisions = variable_revisions.clone();
        let mut variable_ids = Vec::with_capacity(effects.variable_writes.len());
        for effect in &effects.variable_writes {
            let resource = effect.grant.resource.clone();
            let Ok(ResourceIdentity::Variable(variable_id)) = identify_resource(&resource) else {
                return Err(ProjectEffectCommitError::InvalidVariableEffect { resource });
            };
            let Some(current) = variable_revisions.get(&variable_id).copied() else {
                return Err(ProjectEffectCommitError::ResourceRevisionUnavailable { resource });
            };
            let next_revision = current.revision.checked_next().map_err(|_| {
                ProjectEffectCommitError::VariableRevisionExhausted {
                    resource: resource.clone(),
                }
            })?;
            next_data
                .variables
                .insert(variable_id, effect.value.clone());
            next_variable_revisions.insert(
                variable_id,
                crate::project_state::VariableRevisionEntry::present(next_revision),
            );
            variable_ids.push(variable_id);
        }

        let lifecycle_at_gate = self.activation_generation.load(Ordering::Acquire);
        if lifecycle_before != lifecycle_at_gate || !lifecycle_at_gate.is_multiple_of(2) {
            return Err(ProjectEffectCommitError::StaleProjectSession);
        }
        let publication_revision = if effects.variable_writes.is_empty() {
            publication.resource_revision
        } else {
            let advance = publication
                .prepare_resource_revision()
                .map_err(|_| ProjectEffectCommitError::ProjectUnavailable)?;
            *data = next_data;
            *variable_revisions = next_variable_revisions;
            publication.commit_prepared(advance)
        };
        Ok(CommittedProjectEffects {
            project_instance_id: authority.session.instance_id,
            authority_generation: publication.authority_generation(),
            publication_revision,
            resource_grants: effects.grants,
            variable_ids: variable_ids.into_boxed_slice(),
        })
    }
}

fn check_commit_control(
    control: &ProjectEffectCommitControl,
) -> Result<(), ProjectEffectCommitError> {
    if control.cancellation.load(Ordering::Acquire) {
        return Err(ProjectEffectCommitError::Cancelled);
    }
    if Instant::now() >= control.deadline {
        return Err(ProjectEffectCommitError::DeadlineExceeded);
    }
    Ok(())
}
