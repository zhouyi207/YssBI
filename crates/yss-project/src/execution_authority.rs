use std::collections::{BTreeSet, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use thiserror::Error;

use crate::{MutationPublication, ProjectSession, ProjectState};
use yss_filesystem::NormalizedRoot;

use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_project_model::ProjectData;

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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProjectResourceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProjectResourceKind {
    DataFrame,
    File,
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
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn from_revision(revision: ResourceRevision) -> Self {
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

    pub const fn access(&self) -> ProjectResourceAccess {
        self.access
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
    document: Arc<GraphDocument>,
    resource_grants: Arc<[ProjectResourceGrant]>,
}

pub struct PreparedProjectExecution {
    authority: ProjectExecutionAuthority,
    resources: ProjectExecutionResourceSnapshot,
}

impl PreparedProjectExecution {
    pub fn authority(&self) -> &ProjectExecutionAuthority {
        &self.authority
    }

    pub fn resources(&self) -> &ProjectExecutionResourceSnapshot {
        &self.resources
    }
}

#[derive(Clone)]
pub struct ProjectExecutionConfirmationControl {
    cancellation: Arc<AtomicBool>,
    deadline: Instant,
}

impl ProjectExecutionConfirmationControl {
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
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ProjectExecutionConfirmationError {
    #[error("project execution authority is unavailable")]
    ProjectUnavailable,
    #[error("project execution authority is stale")]
    StaleProjectSession,
    #[error("project graph document changed during execution")]
    GraphChanged,
    #[error("project execution dependency is unavailable")]
    ResourceUnavailable { resource: ProjectResourceId },
    #[error("project execution dependency has no revision authority")]
    ResourceRevisionUnavailable { resource: ProjectResourceId },
    #[error("project execution dependency kind changed")]
    ResourceKindChanged { resource: ProjectResourceId },
    #[error("project execution dependency presence changed")]
    ResourcePresenceChanged { resource: ProjectResourceId },
    #[error("project execution dependency version changed")]
    ResourceVersionChanged { resource: ProjectResourceId },
    #[error("project execution confirmation was cancelled")]
    Cancelled,
    #[error("project execution confirmation deadline was exceeded")]
    DeadlineExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceFamilyKind {
    Database,
    File,
}

enum ResourceIdentity {
    Database(Box<str>),
    File(GraphResourcePath),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceResolutionFailure {
    InvalidIdentity,
    UnknownResource,
    KindMismatch { actual: ProjectResourceKind },
    Unavailable,
    RevisionUnavailable,
}

fn identify_resource(
    resource: &ProjectResourceId,
) -> Result<ResourceIdentity, ResourceResolutionFailure> {
    let raw = resource.as_str();
    if let Some(id) = yss_database_contract::database_id_in_resource_path(raw) {
        return Ok(ResourceIdentity::Database(id.into()));
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
    }
}

fn validate_requested_kind(
    identity: &ResourceIdentity,
    requested: ProjectResourceKind,
) -> Result<(), ResourceResolutionFailure> {
    let actual = match resource_family_kind(identity) {
        ResourceFamilyKind::Database => ProjectResourceKind::DataFrame,
        ResourceFamilyKind::File => ProjectResourceKind::File,
    };
    if requested == actual {
        return Ok(());
    }
    Err(ResourceResolutionFailure::KindMismatch { actual })
}

fn resource_grant_from_requirement(
    requirement: &ProjectResourceRequirement,
    data: &ProjectData,
    graph_resource_revisions: &HashMap<GraphResourcePath, ResourceRevision>,
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
            let version = graph_resource_revisions
                .get(&path)
                .copied()
                .map(ProjectResourceVersion::from_revision);
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
) -> ProjectExecutionConfirmationError {
    match failure {
        ResourceResolutionFailure::RevisionUnavailable => {
            ProjectExecutionConfirmationError::ResourceRevisionUnavailable {
                resource: resource.clone(),
            }
        }
        ResourceResolutionFailure::Unavailable
        | ResourceResolutionFailure::InvalidIdentity
        | ResourceResolutionFailure::UnknownResource => {
            ProjectExecutionConfirmationError::ResourceUnavailable {
                resource: resource.clone(),
            }
        }
        ResourceResolutionFailure::KindMismatch { .. } => {
            ProjectExecutionConfirmationError::ResourceKindChanged {
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
    let root = NormalizedRoot::from_path(crate::project_root_from_path(path)).map_err(|_| ())?;
    Ok(ProjectSession {
        instance_id: ProjectInstanceId::from_existing(publication.project_instance_id.clone()),
        root,
    })
}

fn compare_current_grant(
    expected: &ProjectResourceGrant,
    current: &ProjectResourceGrant,
) -> Result<(), ProjectExecutionConfirmationError> {
    if expected.kind != current.kind {
        return Err(ProjectExecutionConfirmationError::ResourceKindChanged {
            resource: expected.resource.clone(),
        });
    }
    if expected.presence != current.presence {
        return Err(ProjectExecutionConfirmationError::ResourcePresenceChanged {
            resource: expected.resource.clone(),
        });
    }
    if expected.version != current.version {
        return Err(ProjectExecutionConfirmationError::ResourceVersionChanged {
            resource: expected.resource.clone(),
        });
    }
    Ok(())
}

struct CurrentAuthorityContents<'a> {
    publication: &'a MutationPublication,
    project_path: &'a Option<String>,
    identity: &'a crate::project_state::ProjectAuthorityExpectation,
    data: &'a ProjectData,
    graph_resource_revisions: &'a HashMap<GraphResourcePath, ResourceRevision>,
    database_revisions: &'a HashMap<String, u64>,
}

fn validate_current_authority_contents(
    authority: &ProjectExecutionAuthority,
    current: CurrentAuthorityContents<'_>,
) -> Result<(), ProjectExecutionConfirmationError> {
    let CurrentAuthorityContents {
        publication,
        project_path,
        identity,
        data,
        graph_resource_revisions,

        database_revisions,
    } = current;
    let session = current_project_session(publication, project_path)
        .map_err(|_| ProjectExecutionConfirmationError::StaleProjectSession)?;
    if session != authority.session
        || identity.project_instance_id != authority.session.instance_id
        || identity.project_root.as_ref() != Some(&authority.session.root)
    {
        return Err(ProjectExecutionConfirmationError::StaleProjectSession);
    }
    if publication.project_instance_id != authority.session.instance_id.as_str() {
        return Err(ProjectExecutionConfirmationError::StaleProjectSession);
    }
    let graph = data
        .graphs
        .get(&authority.graph_path)
        .ok_or(ProjectExecutionConfirmationError::StaleProjectSession)?;
    if graph.document != authority.document {
        return Err(ProjectExecutionConfirmationError::GraphChanged);
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
            graph_resource_revisions,
            database_revisions,
        )
        .map_err(|failure| current_resolution_error(&expected.resource, failure))?;
        compare_current_grant(expected, &current)?;
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
        let graph_resource_revisions = self
            .graph_resource_revisions
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
                    &graph_resource_revisions,
                    &database_revisions,
                )
                .map_err(|failure| {
                    preparation_resolution_error(&requirement.resource, failure, requirement)
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let resource_grants: Arc<[ProjectResourceGrant]> = Arc::from(grants);
        let authority = ProjectExecutionAuthority {
            session,
            graph_path: request.graph_path,
            document: Arc::clone(&graph.document),
            resource_grants: Arc::clone(&resource_grants),
        };
        let lifecycle_after = self.activation_generation.load(Ordering::Acquire);
        if lifecycle_before != lifecycle_after || !lifecycle_after.is_multiple_of(2) {
            return Err(ProjectExecutionPreparationError::Unavailable);
        }
        Ok(PreparedProjectExecution {
            authority,
            resources: ProjectExecutionResourceSnapshot {
                grants: resource_grants,
            },
        })
    }

    /// Rejects result publication when the Project facts captured by `prepare_execution`
    /// changed while the run was executing.
    pub fn confirm_execution_authority(
        &self,
        authority: &ProjectExecutionAuthority,
        control: &ProjectExecutionConfirmationControl,
    ) -> Result<(), ProjectExecutionConfirmationError> {
        check_confirmation_control(control)?;
        self.ensure_project_operational()
            .map_err(|_| ProjectExecutionConfirmationError::ProjectUnavailable)?;
        let lifecycle_before = self.activation_generation.load(Ordering::Acquire);
        if !lifecycle_before.is_multiple_of(2) {
            return Err(ProjectExecutionConfirmationError::StaleProjectSession);
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
        let graph_resource_revisions = self
            .graph_resource_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let database_revisions = self
            .database_authority_revisions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Waiting for the locks may cross the deadline or a cancellation request.
        check_confirmation_control(control)?;
        validate_current_authority_contents(
            authority,
            CurrentAuthorityContents {
                publication: &publication,
                project_path: &project_path,
                identity: &identity,
                data: &data,
                graph_resource_revisions: &graph_resource_revisions,
                database_revisions: &database_revisions,
            },
        )?;
        let lifecycle_after = self.activation_generation.load(Ordering::Acquire);
        if lifecycle_before != lifecycle_after || !lifecycle_after.is_multiple_of(2) {
            return Err(ProjectExecutionConfirmationError::StaleProjectSession);
        }
        Ok(())
    }
}

fn check_confirmation_control(
    control: &ProjectExecutionConfirmationControl,
) -> Result<(), ProjectExecutionConfirmationError> {
    if control.cancellation.load(Ordering::Acquire) {
        return Err(ProjectExecutionConfirmationError::Cancelled);
    }
    if Instant::now() >= control.deadline {
        return Err(ProjectExecutionConfirmationError::DeadlineExceeded);
    }
    Ok(())
}
